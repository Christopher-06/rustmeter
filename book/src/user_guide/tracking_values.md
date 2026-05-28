# Tracking Values

Beyond measuring time, a crucial part of understanding an embedded system is tracking how its state changes over time. The `monitor_value!` macro is designed for exactly this purpose, allowing you to record and graph numerical data.

### What is `monitor_value!` for?

Use this macro to record values that change during your application's runtime. This is perfect for:

-   **Sensor Data:** Temperature, pressure, humidity, accelerometer readings.
-   **System State:** Free memory, battery voltage, CPU load.
-   **Application Metrics:** Number of items in a queue, number of errors occurred, loop counters.

In Perfetto, these values are displayed as **Counter Tracks**, which are graphs that plot the value on the Y-axis against time on the X-axis.

### How to Use It

-   **Syntax:** The macro takes two arguments: a name for the metric (as a string literal) and the value you want to record.

    ```rust
    monitor_value!("metric_name", value);
    ```

-   **Value Types:** The macro accepts integer types like `u32`, `i32`, `u64`, `i64` and floating-point types like `f32` and `f64`.

-   **Example: Tracking Free Memory**
    Let's say you have a function `get_free_heap_size()` that returns the available memory in bytes. You could create a simple task to monitor this value every second.

    ```rust
    #[embassy_executor::task]
    async fn memory_monitor_task() {
        loop {
            let free_memory = get_free_heap_size(); // Your function to get memory
            monitor_value!("free_heap", free_memory as u32);
            Timer::after(Duration::from_secs(1)).await;
        }
    }

    // In your main spawner:
    spawner.spawn(memory_monitor_task()).unwrap();
    ```

### What You Get in Perfetto

This will create a new track in Perfetto named `free_heap`. When you select it, you will see a graph showing how the amount of free memory changes over time. This is incredibly useful for detecting memory leaks or understanding memory usage patterns under different application loads.

### What to Pay Attention To

-   **One Track per Name:** Every call to `monitor_value!` with the same name (e.g., `"free_heap"`) will add a point to the *same* graph. If you want to track different metrics, give them unique names.

    ```rust
    // This creates TWO separate graphs
    monitor_value!("temperature", 25.5);
    monitor_value!("humidity", 45.2); 
    ```

-   **Data Types:** While you can log different numeric types, Perfetto will display them on the same scale. It's generally best to be consistent with the type of data you log to a single track.

-   **Performance:** Just like other monitoring macros, each call to `monitor_value!` has a small performance overhead. Avoid calling it in extremely tight, performance-critical loops unless you are specifically trying to debug that loop's behavior.
