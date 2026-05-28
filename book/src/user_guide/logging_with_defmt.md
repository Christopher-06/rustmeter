# Logging with defmt

Correlating log messages with runtime events is a cornerstone of effective debugging. RustMeter seamlessly integrates with the `defmt` logging framework to display your logs directly on the Perfetto timeline, perfectly synchronized with all other trace data.

### What is the `defmt` Integration for?

This feature allows you to see exactly *when* a log message was emitted in relation to your task's execution state, function calls, and tracked values. It helps answer questions like:

-   Did this error message occur while Task A was running or Task B?
-   How long after a function started did it log a specific warning?
-   What was the value of my sensor right before a critical log message was printed?

### How to Use It

-   **It's Automatic!** There is no special configuration required to enable this feature, besides using `defmt` in your project and having `rustmeter-beacon` initialized.

-   **Prerequisites:**
    1.  Your project must be set up to use `defmt` for logging.
    2.  You must initialize `rustmeter-beacon::init()` in your application.

-   **Example:**
    Simply use the `defmt` logging macros as you normally would.

    ```rust
    #[embassy_executor::task]
    async fn my_task() {
        loop {
            defmt::info!("Starting a new cycle.");
            
            monitor_scoped!("Work", {
                // ... do some work ...
            });

            if some_condition_is_bad() {
                defmt::warn!("A bad condition was detected!");
            }

            Timer::after(Duration::from_secs(1)).await;
        }
    }
    ```

### What You Get in Perfetto

-   Each log message is shown as an **instant event** (a marker) on the core timeline, aligned with the exact timestamp when it was emitted.
-   The log level (e.g., `INFO`, `WARN`, `ERROR`).
-   The full content of the log message.

This provides a powerful, unified view of your system's behavior.

### What to Pay Attention To

-   **No `defmt` Timestamps Required:** A major advantage of this integration is that **you do not need to enable `defmt`'s built-in timestamping feature**. RustMeter provides its own high-precision timestamps for all events, including logs. This simplifies your project setup.

-   **Avoid Conflicting Loggers:** RustMeter's CLI tool (`rustmeter run`) listens for data on a specific communication channel. If you use other logging crates that also try to take exclusive control of that channel (like using `rtt-target` directly or `esp-println`), they will interfere with RustMeter. When tracing with RustMeter, you should rely solely on `defmt` for your logging output.
