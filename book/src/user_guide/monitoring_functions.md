# Monitoring Functions and Scopes

While automatic task tracing is powerful, you often need more granular control to identify performance bottlenecks. RustMeter provides several macros to manually instrument your code.

### `#[monitor_fn]`: Monitoring Entire Functions

This attribute macro wraps an entire function, measuring its total execution time from start to finish.

-   **How to Use:** Add `#[monitor_fn]` directly above a function definition.

-   **Synchronous Functions:** For regular `fn`, it measures the time from function entry to exit.

    ```rust
    #[monitor_fn]
    fn my_blocking_calculation() {
        // some heavy work
        // The entire duration of this function will be a single block in Perfetto
    }
    ```

-   **Asynchronous Functions:** For `async fn`, it gets more interesting. The macro automatically breaks down the function's execution into distinct phases based on `.await` points.
    -   Each segment of code between `.await` calls is shown as a separate `Running` block in Perfetto. See the step! macro for custom naming below your steps!
    -   This allows you to see not just the total time, but which parts of your async function are taking the longest.
    - The idle state will in the future get annoted with the .await point that caused it, so you can easily correlate waiting times with the specific async operations that are causing them!

    ```rust
    #[monitor_fn]
    async fn my_async_operation() {
        // This part is the first execution block
        let data = fetch_data().await; 

        // This part is the second execution block
        process_data(data).await;

        // This is the third
    }
    ```

-   **Custom Naming:** By default, the trace uses the function's name. You can provide a custom name for the trace.

    ```rust
    #[monitor_fn(name = "MyCustomName")]
    fn my_real_function_name() {
        // ...
    }
    ```

### `step!`: Marking Points in Async Functions

Inside an `async` function marked with `#[monitor_fn]`, the `step!` macro allows you to create named markers on the timeline. This is extremely useful for understanding the flow of a complex async function.

-   **How to Use:** Call `step!("your_step_name");` at any point within an `async fn` that is also decorated with `#[monitor_fn]`.

-   **Example:**
    ```rust
    #[monitor_fn]
    async fn complex_async_task() {
        step!("Starting phase 1");
        /* Some code for phase 1 */
        do_part_one().await;

        step!("Starting phase 2");
        /* Some code for phase 2 */
        do_part_two().await;

        step!("Finishing up");
        /* Final code */
    }
    ```
    In Perfetto, this will label "Running (Starting phase 1)", "Running (Starting phase 2)", etc., on the timeline for this function, perfectly aligned with its execution blocks.

- INFO: Be aware of your execution model of the function because a step! in a for loop for example will create a step since it get's called but after exiting the loop, the step will still be opened!

### `monitor_scoped!`: Profiling Arbitrary Code Blocks

Sometimes you don't want to monitor a whole function, but just a specific section of code. `monitor_scoped!` is perfect for this.

-   **How to Use:** Wrap any block of code with `monitor_scoped!("my_scope_name", { ... });`.

-   **Use Cases:**
    -   Measuring the time of a specific loop.
    -   Profiling a calculation within a larger function.
    -   Understanding the performance of a section of code that isn't neatly encapsulated in its own function.

-   **Example:**
    ```rust
    fn some_bigger_function() {
        // ... some code ...

        monitor_scoped!("CriticalLoop", {
            for i in 0..1000 {
                // do something important
            }
        });

        // ... more code ...
    }
    ```

-   **`step!` in `monitor_scoped!`?** **No.** The `step!` macro is designed to work with the state machine of an `async fn` and is only effective inside a function decorated with `#[monitor_fn]`. It will not have any effect inside a `monitor_scoped!` block. Use `monitor_scoped!` for synchronous blocks or for measuring the total time of an async block without breaking it down. But you can use a monitor_scoped! block inside an (async) function that is decorated with #[monitor_fn], and it will nest correctly in the timeline, showing the total time of the block as well as the steps inside it!
