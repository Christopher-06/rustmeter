# User Guide

This guide will walk you through the core features of RustMeter. You'll learn how to use the provided macros and features to gain deep insights into your application's behavior.

We will cover:

-   **Automatic Embassy Task Tracing:** See how RustMeter visualizes the lifecycle of your `embassy` tasks without any manual instrumentation.
-   **Manual Instrumentation:** Learn how to use the `#[monitor_fn]`, `monitor_scoped!`, and `monitor_value!` macros to measure function execution times, profile specific code blocks, and track variable values over time.
-   **Log Correlation:** Understand how `defmt` log messages are automatically integrated into your trace timeline.
-   **Multicore Tracing:** See how to handle and visualize traces from systems with multiple processor cores.

---

### A Note on Performance Overhead

It's important to remember that any form of monitoring or tracing introduces a small amount of overhead. Each time an event is generated (e.g., a function is entered, a value is logged), it takes a tiny amount of time for the `rustmeter-beacon` to record it.

While RustMeter is designed to be extremely lightweight, this overhead can add up if you are monitoring very short, high-frequency events. This means the measurements you see in Perfetto will always include this small instrumentation cost.

For a detailed analysis of this overhead and strategies to minimize its impact, please refer to the [**Performance & Overhead**](../advanced/performance.md) chapter in the Advanced Topics section.
