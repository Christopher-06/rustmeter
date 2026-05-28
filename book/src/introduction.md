# RustMeter: Usage Guide and Reference


## What is RustMeter?

- **What is RustMeter?**
  - A comprehensive profiling, tracing, and monitoring system.
  - Specifically designed for **Embedded Rust** applications.
  - Highly integrated with the `embassy` async framework and `defmt` logging system.

- **What is its purpose?**
  - To provide developers with deep insights into the runtime behavior of their embedded firmware.
  - To visualize application performance, task scheduling, and resource utilization.
  - To make debugging complex, real-time interactions in async embedded systems easier.

- **What problems does it solve?**
  - **Lack of Visibility:** Embedded systems, especially async ones, can be a "black box". RustMeter opens up this box to show you what's really happening.
  - **Performance Bottlenecks:** Identify which functions or tasks are taking too long and slowing down your application.
  - **Task Scheduling Issues:** Understand how your async tasks are being scheduled, where they are waiting, and why they might not be running as expected (e.g., starvation).
  - **Correlation of Events:** Correlate log messages (`defmt`) with specific task activities and performance metrics on a single timeline.
  - **Resource Monitoring:** Track custom metrics like memory usage, sensor values, or power consumption over time to understand their impact on the system.


## Core Concepts

- **Dual-Component Architecture:**
    - **`rustmeter-beacon`**: A lightweight Rust library (`crate`) that lives inside your embedded firmware.
        - It provides macros (`#[monitor_fn]`, `monitor_scoped!`, `monitor_value!`) to instrument your code.
        - It hooks into the `embassy` executor to automatically capture task lifecycle events (spawned, running, waiting, idle).
        - It captures `defmt` log messages.
        - It uses a highly efficient, lock-free ring buffer (`per-core`) to store event data with minimal runtime overhead.
        - A background task sends the collected data from the ring buffer over a communication channel (like RTT or Serial).
    - **`rustmeter-cli`**: A command-line tool for your development PC.
        - It acts as the host-side counterpart.
        - It runs your embedded application (e.g., via `probe-rs` or `espflash`).
        - It listens for the data stream sent by the `rustmeter-beacon`.
        - It decodes the raw data stream.
        - It converts the decoded events into the `Perfetto` JSON trace format.

- **Data Flow:**
    1.  **Instrumentation:** You add `rustmeter` macros to your embedded code.
    2.  **Event Generation:** As your firmware runs, the macros and `embassy` hooks generate trace events (function calls, task state changes, etc.).
    3.  **Buffering:** These events are quickly written into a lock-free ring buffer on the target MCU, minimizing blocking of your application code.
    4.  **Transmission:** A low-priority background task reads events from the buffer and transmits them to the host PC via RTT or another channel.
    5.  **Collection & Conversion:** The `rustmeter-cli` on the host captures this stream of bytes.
    6.  **Visualization:** The CLI processes the data and saves it as a `.json` file.
    7.  **Analysis:** You open this JSON file in the [Perfetto UI](https://ui.perfetto.dev/) to visualize and analyze the entire trace.

- **Core Principle: Low Overhead:**
    - The system is designed to be as non-intrusive as possible.
    - Using lock-free data structures and background data transmission minimizes the impact on the real-time performance of your application. This is crucial for embedded systems where timing is critical.
