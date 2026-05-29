# RustMeter Beacon

**The embedded instrumentation library for the RustMeter profiling system.**

[**`rustmeter-beacon`**](https://crates.io/crates/rustmeter-beacon) serves as the device-side component for RustMeter. It sits directly inside your embedded firmware, silently capturing runtime events, performance metrics, and task transitions with ultra-low overhead. 

<p align="center">
  <img src="https://raw.githubusercontent.com/Christopher-06/rustmeter/refs/heads/main/ressources/perfetto-ui-esp32-multicore.png" alt="Perfetto UI Screenshot" />
</p>

## ✨ Features

- **Zero-Boilerplate Embassy Integration**: Hooks deeply into the `embassy-executor` to automatically trace async task states (Spawn, Wait, Run, Idle).
- **Macro-Driven Tracing**: Easily instrument critical functions with `#[monitor_fn]` or measure specific code blocks loops with `monitor_scoped!`.
- **Sensor Telemetry**: Emit custom runtime metrics and variables using `monitor_value!` to visualize them as live graphs alongside your code execution.
- **Log Correlation**: Fully integrates with `defmt`, allowing your log outputs to act as pinpoint events on the tracing timeline.
- **Multi-Core Ready**: Tracks and visually separates tasks executed across different CPU cores on supported hardware.

## 🛠️ How it works

The beacon utilizes lock-free ringbuffers per core to rapidly enqueue event metadata (timestamps, string pointers). A low-priority background task routinely flushes this buffer out of the microcontroller (via RTT or UART), ensuring that the overhead on the critical path of your application remains minimal.

## 📖 Installation & Usage

To keep our documentation localized, fully up-to-date, and easy to read, we host all setup instructions, API usage examples, and architecture details centrally. 

Discover how to add `rustmeter-beacon` to your `Cargo.toml`, initialize the system, and instrument your code:

👉 **[Read the RustMeter Book](https://christopher-06.github.io/rustmeter/)**

