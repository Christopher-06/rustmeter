# Performance & Overhead

While RustMeter is designed to be as lightweight as possible, it's crucial to understand that any form of instrumentation introduces a performance overhead. This chapter explains what that overhead is, where it comes from, and how to interpret your results with it in mind.

### The Heisenberg Principle of Profiling

You can't observe a system without affecting it. Every time a `rustmeter-beacon` macro is called (`#[monitor_fn]`, `monitor_scoped!`, `monitor_value!`, `step!`, or even an automatic `embassy` event), your program executes a few extra instructions to record that event.

This means that the execution times you see in Perfetto are the **real execution time of your code PLUS the time it took to record the event**.

### Measured Overhead

The overhead varies depending on the target architecture, its clock speed, and memory access times. Here are the current benchmarked values:

-   **Espressif MCUs (e.g., ESP32, ESP32-S3):**
    -   **Overhead per event: ~0.4 µs to 1.2 µs**
    -   These chips benefit from faster clock speeds, instruction caches, and optimized atomic operations, resulting in a very low overhead.

-   **Cortex-M MCUs (e.g., STM32, RP2040):**
    -   **Overhead per event: ~1 µs to 8 µs**
    -   The overhead is currently higher on these targets due to generally lower clock frequencies and the lack of certain hardware-accelerated features. Future optimizations aim to reduce this significantly.

### Where Does the Overhead Come From?

RustMeter's design minimizes the time your application is blocked. When an event occurs:

1.  The event data (a timestamp and a few bytes of data) is generated.
2.  This data is written into a **lock-free per-core ring buffer** (`ring-buffer`). This is a highly efficient, wait-free data structure that allows the application to "fire and forget" the event data very quickly. This is where the overhead mentioned above is incurred.
3.  A separate, lower-priority background task is responsible for reading data from this buffer and sending it to the host PC via RTT or Serial. Because this runs in the background, it doesn't block your main application logic. But if this task get's blocked, it can cause the buffer to fill up, which could lead to dropped events if the application generates events faster than they can be sent out. Or Deadlock Situations. FOr this, we try to implement a blocking feature to allow any event to directly write to the output channel. Normally this would be more expensive per call (more Overhead) but it would guarantee that no events get lost in such situations like Bufferoverlow or Deadlocking. This is currently not implemented, but it's on the roadmap.

The key is that your application only has to wait for the very fast write to the ring buffer.

### How to Interpret Your Traces

-   **Focus on Relative, Not Absolute Times:** If function `A` takes 50 µs and function `B` takes 200 µs in your trace, the key insight is that `B` is four times slower than `A`. The exact absolute values (e.g., 50 µs) are slightly inflated by the overhead, but the *ratio* between them is accurate.

-   **Be Mindful of High-Frequency Events:** If you are monitoring a function that runs in just a few microseconds, the relative overhead will be high. For a function that takes 5 µs to run on an STM32, an overhead of 2-3 µs is significant. For a function that takes 500 µs, the same overhead is negligible.

-   **Use Instrumentation Judiciously:** Don't wrap every single line in a `monitor_scoped!`. Start by monitoring larger functions (`#[monitor_fn]`) and then add more granular scopes only where you need to dig deeper. This helps keep the overall overhead low and your traces clean.

For most applications, the insights gained from seeing the interactions between tasks and functions far outweigh the small cost of the instrumentation.
