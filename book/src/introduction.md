# Introduction

RustMeter is a comprehensive profiling, tracing, and monitoring toolkit designed exclusively for Embedded Rust applications. It integrates seamlessly with the `embassy` asynchronous framework and the popular `defmt` logging ecosystem.

In the challenging world of embedded systems, it can feel like your software is trapped inside a "black box" once it's flashed to the target microcontroller. We built RustMeter to tear that box open. With RustMeter, you can visually trace exactly how your code takes shape over time, helping you uncover performance bottlenecks, identify scheduling bugs, and correlate obscure sensor anomalies at a glance.

### Why Do I Need It?

When developing deeply embedded firmware—especially complex asynchronous systems—you run into several specific challenges that RustMeter addresses directly:

- **Lack of Visibility:** It's notoriously hard to trace execution flows inside microcontrollers without massive debug probing overhead. RustMeter provides a clean, visual representation of what's currently executing.
- **Performance Bottlenecks:** It helps you clearly spot which exact functions or async tasks consume an unusual amount of processor execution time, slowing everything else down.
- **Task Scheduling Issues:** Are tasks taking too long to yield? Are they starving each other, or perhaps mysteriously stuck waiting in queues without executing? RustMeter visualizes precisely how the `embassy` scheduler routes priority.
- **Correlation of Events:** Have you ever wondered what the exact state of a system was right before a `defmt` fault logged on your screen? RustMeter places those log messages natively on exactly the same timeline as everything else.
- **Resource Monitoring:** Whether it’s observing raw free heap memory, capturing active sensor readings over I2C, or plotting power consumption metrics, RustMeter graphs your custom dynamic properties securely across time.

### Core Concepts

RustMeter adopts an extremely effective dual-component structure to keep the embedded overhead safely bounded while providing enormous power on the host side:

1. **`rustmeter-beacon` (The Embedded Side):** This is a lightweight Rust library crated designed to inhabit your embedded application. It packs handy macros such as `#[monitor_fn]`, `monitor_scoped!`, and `monitor_value!` so that you can easily tag the logic you want to observe. It also hooks cleanly into the internal logic of the `embassy` executor. 
   When your application runs, this beacon accumulates trace events into extremely fast, lock-free ring buffers (isolated per core!). A dedicated low-priority background task gracefully empties this buffer and squirts the data packet over a channel like RTT or Serial to your host.
   
2. **`rustmeter-cli` (The Host Side):** This is the command line application residing on your development machine. The tool effortlessly connects to your target (by proxying tools such as `probe-rs` or `espflash`), seamlessly catching the data arriving from your `rustmeter-beacon`.
   Using metadata uniquely baked into your firmware's ELF binary, it decodes this byte stream into a human-readable, perfectly formatted `Perfetto` JSON trace.

### The Data Journey

If you’re wondering how this all connects chronologically, here's the typical lifecycle of trace data:

First, your firmware runs your specialized functions augmented with our macros. These embedded routines then generate continuous timestamped events. Behind the scenes, we quickly dispatch these raw data slices directly into a lock-free internal ring buffer, making absolutely sure we don't interfere with or block your critical paths. 
A low priority backend thread eventually transmits this buffer out of the system. Waiting on your PC, the `rustmeter-cli` receives it, decodes it referencing local metadata, and outputs a standard JSON file.
Finally, all that's left to do is upload that JSON visually to the beautiful [Perfetto UI](https://ui.perfetto.dev/) timeline viewer exactly as they use for large-scale Chromium traces!

We proudly enforce a **Core Principle of Low Overhead**. By intelligently exploiting lock-free asynchronous buffers, the delay we impart onto your active logic is breathtakingly tiny. This ensures that profiling with RustMeter doesn't mask the very time-critical performance quirks you strictly aim to uncover!
