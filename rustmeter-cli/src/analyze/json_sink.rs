use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
};

use polars::prelude::*;
use serde_json::Value;

/// Simple JSON sink that writes Parquet files into a single JSON file. Used for perfetto
/// trace export or other JSON-based analysis.
pub struct JsonSink {
    file: File,
    source: Vec<std::path::PathBuf>,
}

impl JsonSink {
    /// Create a new JsonSink from multiple source files
    pub fn new(
        filename: std::path::PathBuf,
        source: Vec<std::path::PathBuf>,
    ) -> anyhow::Result<Self> {
        let file = File::create(&filename)?;
        Ok(Self { file, source })
    }

    /// Create a new JsonSink from a single source file
    pub fn new_single(
        filename: std::path::PathBuf,
        source: std::path::PathBuf,
    ) -> anyhow::Result<Self> {
        JsonSink::new(filename, vec![source])
    }

    /// Create a new JsonSink from all parquet files in a folder
    pub fn new_folder(
        filename: std::path::PathBuf,
        source_folder: std::path::PathBuf,
    ) -> anyhow::Result<Self> {
        let mut source = Vec::new();
        for entry in std::fs::read_dir(source_folder)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "parquet") {
                source.push(path);
            }
        }

        JsonSink::new(filename, source)
    }

    /// Sink all source files into a single JSON file
    pub fn finish(mut self) -> anyhow::Result<()> {
        let mut writer = BufWriter::new(&mut self.file);
        writer.write_all(b"[\n")?;

        for filename in &self.source {
            // Read DataFrame
            let path = filename.to_string_lossy().to_string();
            let lf =
                LazyFrame::scan_parquet(PlPath::from_string(path), ScanArgsParquet::default())?;
            let mut df = lf.collect()?;
            df.rechunk_mut();

            // Write as JSON
            write_as_json(&mut writer, df)?;
        }

        writer.write_all(b"]")?;
        Ok(())
    }
}

fn write_as_json<W: Write>(writer: &mut W, df: DataFrame) -> anyhow::Result<()> {
    // Prepare iterators
    let fields = df.fields();
    let columns: Vec<&str> = fields.iter().map(|x| x.name().as_str()).collect();
    let mut iters = df.iter().map(|s| s.iter()).collect::<Vec<_>>();

    // Go through all rows
    for _ in 0..df.height() {
        let mut row = HashMap::new();
        for (column, iter) in std::iter::zip(&columns, &mut iters) {
            let value = iter.next().expect("should have as many iterations as rows");

            // Add any non-null value
            if let Some(value) = anyvalue_to_json(&value) {
                row.insert(column.to_string(), value);
            }
        }

        // write non-empty rows
        if !row.is_empty() {
            let json_str = serde_json::to_string(&row)?;
            writer.write_all(json_str.as_bytes())?;
            writer.write_all(b",\n")?;
        }
    }

    Ok(())
}

fn anyvalue_to_json(av: &AnyValue) -> Option<Value> {
    match av {
        AnyValue::Null => None,
        AnyValue::Boolean(b) => Some(Value::from(*b)),
        AnyValue::UInt8(v) => Some(Value::from(*v)),
        AnyValue::UInt16(v) => Some(Value::from(*v)),
        AnyValue::UInt32(v) => Some(Value::from(*v)),
        AnyValue::UInt64(v) => Some(Value::from(*v)),
        AnyValue::Int8(v) => Some(Value::from(*v)),
        AnyValue::Int16(v) => Some(Value::from(*v)),
        AnyValue::Int32(v) => Some(Value::from(*v)),
        AnyValue::Int64(v) => Some(Value::from(*v)),
        AnyValue::Float32(v) => Some(Value::from(*v as f64)),
        AnyValue::Float64(v) => Some(Value::from(*v)),
        AnyValue::String(s) => Some(Value::from(s.to_string())),
        AnyValue::StringOwned(s) => Some(Value::from(s.to_string())),
        AnyValue::List(series) => {
            let json_list: Vec<Value> = series
                .iter()
                .map(|x| anyvalue_to_json(&x).unwrap_or(Value::Null))
                .collect();

            Some(Value::Array(json_list))
        }
        AnyValue::Struct(.., fields) => {
            let mut buf = Vec::new();
            av._materialize_struct_av(&mut buf);

            // Get all key-value pairs without NULL
            let mut map = serde_json::Map::new();
            for (f, val) in fields.iter().zip(buf.iter()) {
                let key = f.name().to_string();
                if let Some(json_val) = anyvalue_to_json(val) {
                    map.insert(key, json_val);
                }
            }

            if map.is_empty() {
                return None;
            }
            Some(Value::Object(map))
        }
        AnyValue::StructOwned(s) => {
            let (vals, fields) = s.as_ref();

            // Get all key-value pairs without NULL
            let mut map = serde_json::Map::new();
            for (f, val) in fields.iter().zip(vals.iter()) {
                let key = f.name().to_string();
                if let Some(json_val) = anyvalue_to_json(val) {
                    map.insert(key, json_val);
                }
            }

            if map.is_empty() {
                return None;
            }
            Some(Value::Object(map))
        }
        _ => None, // Skip other types
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::*;

    fn test_frame() -> DataFrame {
        df![
            "a" => &[Some(1u32), Some(2u32), None, Some(3u32), Some(4u32), Some(5u32)],
            "b" => &[
                Some("hello"),
                None,
                Some("test"),
                None,
                Some("rust"),
                Some("polars")
            ],
            "c" => &[
                Some(3.14f64),
                None,
                None,
                None,
                Some(0.0f64),
                None
            ],
            "d" => &[
                None,
                Some(false),
                None,
                None,
                Some(true),
                Some(true)
            ]
        ]
        .unwrap()
        .lazy()
        .with_column(
            as_struct(vec![
                col("a").alias("a"),
                col("b").alias("b"),
                col("c").alias("c"),
                col("d").alias("d"),
            ])
            .alias("args"),
        )
        .collect()
        .unwrap()
    }

    #[test]
    fn test_write_as_json() {
        // Serialize test frame
        let df = test_frame();
        let mut buf: Vec<u8> = Vec::new();
        write_as_json(&mut buf, df).unwrap();

        // Check output
        let output_string: String =
            "[".to_string() + &String::from_utf8_lossy(&buf[..buf.len() - 2]) + "]";
        let json_values: Vec<Value> = serde_json::from_str(&output_string).unwrap();

        // Assert them
        assert_eq!(json_values.len(), 6);
        assert_eq!(
            json_values[0],
            serde_json::json!({"a": 1, "b": "hello", "c": 3.14, "args": { "a": 1, "b": "hello", "c": 3.14 }})
        );
        assert_eq!(
            json_values[1],
            serde_json::json!({"a": 2, "d": false, "args": { "a": 2, "d": false }})
        );
        assert_eq!(
            json_values[2],
            serde_json::json!({"b": "test", "args": { "b": "test" }})
        );
        assert_eq!(
            json_values[3],
            serde_json::json!({"a": 3, "args": { "a": 3 }})
        );
        assert_eq!(
            json_values[4],
            serde_json::json!({"d":true,"c":0.0,"args":{"a":4,"b":"rust","c":0.0,"d":true},"b":"rust","a":4})
        );
        assert_eq!(
            json_values[5],
            serde_json::json!({"a": 5, "b": "polars", "d": true, "args": { "a": 5, "b": "polars", "d": true }})
        );
    }

    #[test]
    fn test_anyvalue_to_json() {
        // Test various AnyValue types
        assert_eq!(anyvalue_to_json(&AnyValue::Null), None);
        assert_eq!(
            anyvalue_to_json(&AnyValue::Boolean(true)),
            Some(Value::from(true))
        );
        assert_eq!(
            anyvalue_to_json(&AnyValue::Int32(42)),
            Some(Value::from(42))
        );
        assert_eq!(
            anyvalue_to_json(&AnyValue::Float64(3.14)),
            Some(Value::from(3.14))
        );
        assert_eq!(
            anyvalue_to_json(&AnyValue::String("hello".into())),
            Some(Value::from("hello"))
        );

        // Test List
        let list = AnyValue::List(Series::new(
            "ggfg".into(),
            &[
                AnyValue::Int32(1),
                AnyValue::Int32(2),
                AnyValue::Null,
                AnyValue::Int32(3),
            ],
        ));
        assert_eq!(
            anyvalue_to_json(&list),
            Some(Value::Array(vec![
                Value::from(1),
                Value::from(2),
                Value::Null,
                Value::from(3)
            ]))
        );

        // Test Struct Owned
        let struct_av = AnyValue::StructOwned(Box::new((
            vec![AnyValue::Int32(10), AnyValue::String("world".into())],
            vec![
                Field::new("a".into(), DataType::Int32),
                Field::new("b".into(), DataType::String),
            ],
        )));
        let expected_map = serde_json::json!({"a": 10, "b": "world"});
        assert_eq!(anyvalue_to_json(&struct_av), Some(expected_map));

        // Test Struct with NULL values
        let struct_av_null = AnyValue::StructOwned(Box::new((
            vec![AnyValue::Int32(10), AnyValue::Null],
            vec![
                Field::new("a".into(), DataType::Int32),
                Field::new("b".into(), DataType::UInt16),
            ],
        )));
        let expected_map_null = serde_json::json!({"a": 10});
        assert_eq!(anyvalue_to_json(&struct_av_null), Some(expected_map_null));

        // Test empty Struct
        let struct_av_empty = AnyValue::StructOwned(Box::new((vec![], vec![])));
        assert_eq!(anyvalue_to_json(&struct_av_empty), None);

        // Test struct with only NULLs
        let struct_av_only_null = AnyValue::StructOwned(Box::new((
            vec![AnyValue::Null, AnyValue::Null],
            vec![
                Field::new("a".into(), DataType::Int32),
                Field::new("b".into(), DataType::UInt16),
            ],
        )));
        assert_eq!(anyvalue_to_json(&struct_av_only_null), None);
    }
}
