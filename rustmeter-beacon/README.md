# RustMeter Beacon

**The embedded instrumentation library for the RustMeter profiling system.**

[`rustmeter-beacon`](https://crates.io/crates/rustmeter-beacon) is a lightweight tracing library designed for embedded Rust applications. It serves as the device-side component that captures runtime events, performance metrics, and task transitions.

It is built to integrate seamlessly with the [Embassy](https://github.com/embassy-rs/embassy) async executor and logging via [defmt](https://github.com/knurling-rs/defmt).

## ✨ Features

- **Embassy Integration**: Hooks into `embassy-executor` to trace task states (Spawn, Run, Wait, Idle).
- **Function Monitoring**: Easily instrument critical functions with the `#[monitor_fn]` attribute.
- **Scoped Tracing**: Measure execution time of specific blocks or loops with `monitor_scoped!`.
- **Custom Metrics**: Log sensor data or internal state variables to visualize them over time with `monitor_value!`.
- **Multi-Core Ready**: Identifies which core code is running on (currently tested on ESP32 Xtensa & RISC-V + RP2040).

## 📦 Installation

Add the crate to your embedded project's `Cargo.toml`. You must speficy the target chip feature to include the appropriate platform-specific implementation. Following chips are currently supported:
- Espressif: `esp32`, `esp32c2`, `esp32c3`, `esp32c6`, `esp32h2`, `esp32s2`, `esp32s3`
- Cortex: `stm32` (generic for all), `rp2040`, `rp235xa` or `rp235xb` (both currently not tested)

When using with defmt, ensure you also activate the `defmt` feature. This will include a global logger implementation required for emitting the trace data.



```toml
[dependencies]
rustmeter-beacon = { version = "X", features = ["defmt", "<CHIP>"] }

# Important: Enable the 'trace' feature!
embassy-executor = { version = "X", features = ["trace", ... ] }

# Remove defmt-rtt and esp-println to avoid conflicts
defmt-rtt = "X"    # <- REMOVE THIS
esp-println = "X"  # <- REMOVE THIS
```

**Note:** `rustmeter-beacon` uses an own RTT logging implementation on cortex controllers. Therefore, you should **not** include the `defmt-rtt` crate in your project to avoid conflicts.

**Note:** `rustmeter-beacon` instantiates an own UART / Serial JTAG logger on Espressif targets. Therefore, you should **not** use the `esp-println` crate in your project to avoid conflicts. Use defmt for printing instead. This will automatically choose which route to use.

## 🛠️ Usage

1. Setup

Simply initialize the beacon once with the system frequency and an executor to spawn the internal tracing task. This task has to be polled regularly to flush the system trace data! You can use any spawner (Thread or Interrupt) on any core depending on your architecture.

```rust
use rustmeter_beacon::*;

fn main() {
    // other setup

    // espressif based:
    let config = esp_hal::Config::default() // <- in your setup
        .with_cpu_clock(CpuClock::max());  // any clock speed
    init_rustmeter_beacon(
        RustmeterConfig::new(config.cpu_clock().frequency()),
        &spawner,
    )

    // cortex based:
    init_rustmeter_beacon(
        RustmeterConfig::new(get_system_freq!()), &spawner
    );

    // Your application code
}
```

2. Instrument Functions

Use the #[monitor_fn] attribute to trace the start and end of a function. 

```rust
#[monitor_fn] 
fn process_data() {
    // ... heavy lifting
}
```

3. Trace Scopes

For more granular control, use the monitor_scoped! macro to measure specific code blocks.

```rust
fn complicated_calculation() {
    // ... setup

    let result = monitor_scoped!("matrix_mul", {
        // This block will be timed separately
        matrix_a * matrix_b
    });

    // ... teardown
}
```

4. Record Metrics

Visualize values over time (like battery voltage, memory usage, or temperature) using monitor_value!. These appear as counter graphs in the trace viewer.

```rust
let temp = temp_sensor.read();
monitor_value!("temperature", temp);
```

5. Defmt Integration

When defmt is enabled, rustmeter-beacon automatically hooks into defmt's global logger to emit trace data. Just use defmt macros as usual:

```rust
defmt::info!("System initialized");
```

## ⚙️ Architecture Support

In theory this crate should work seamlessly on these architectures:
- **Espressif**: ESP32, ESP32-C2, ESP32-C3, ESP32-C6, ESP32-H2, ESP32-S2, ESP32-S3
- **Cortex-M**: Generic support for all Cortex-M based MCUs

In the examples you can find tested implementations for:
- **Espressif**: ESP32 and ESP32-C3
- **Cortex-M**: STM32F446 and RP2040

## 📄 License

This project is licensed under the MIT License.
