# User Guide

This comprehensive User Guide is designed to carefully walk you through the core, powerful features of RustMeter. Along the way, we will show you exactly how to utilize the library's provided macros, ensuring you gain deep, unparalleled insights into your embedded application's inner mechanisms. 

Whether you're struggling to understand exactly what your asynchronous workloads are doing or you just want to track a crucial voltage sensor seamlessly, this guide covers the solutions.

In this section, we will cover:

- **Automatic Embassy Task Tracing:** Discover how RustMeter visualizes the entire lifecycle of your `embassy` tasks perfectly out-of-the-box, without requiring you to manually instrument them. 
- **Manual Instrumentation:** While automatic tracing is incredible, sometimes you need to dig deeper. Learn how to explicitly use the `#[monitor_fn]`, `monitor_scoped!`, and `monitor_value!` macros to analyze exact function execution times, profile custom code blocks, and graph variable behavior over time.
- **Log Correlation:** Say goodbye to endless scrolling purely to align terminal printouts. See how `defmt` log messages are naturally injected directly into your visual trace timeline.
- **Multicore Tracing:** Expand your boundaries further by seeing how to intelligently trace, handle, and visualize performance spanning across systems carrying multiple processor cores dynamically.

---

### A Note on Performance Overhead

When stepping into profiling, it is crucial to understand that any form of software observation inherently introduces a very slight performance overhead. Quite simply, every time an event occurs (like jumping into a monitored function or saving a trackable value), the `rustmeter-beacon` takes a microscopic amount of time to actively write it down.

We spent tremendous effort engineering RustMeter to be exceptionally lightweight so you barely feel a difference. However, this small amount of overhead gradually adds up if you try observing hundreds of extremely short, high-frequency routines simultaneously. Ultimately, this means the execution blocks rendered inside Perfetto are guaranteed to include this tiny instrumentation footprint. 

For an extensive, detailed analysis on overhead metrics and proven strategies applied by experts to minimize it, please feel completely free to jump over to the [Performance & Overhead](./advanced/performance.md) chapter within our Advanced Topics.
