# RustMeter

**RustMeter** is a comprehensive profiling, tracing, and monitoring toolkit designed specifically for **Embedded Rust** applications. It pierces the "black box" of your microcontroller, seamlessly integrating with the [Embassy](https://github.com/embassy-rs/embassy) async framework and the [defmt](https://github.com/knurling-rs/defmt) logging ecosystem. 

With RustMeter, you can visualize the exact life cycle of your firmware on a high-resolution timeline within the **[Perfetto UI](https://ui.perfetto.dev/)**, helping you uncover performance bottlenecks, scheduling conflicts, or sensor anomalies.

<p align="center">
  <img src="https://raw.githubusercontent.com/Christopher-06/rustmeter/refs/heads/main/ressources/perfetto-ui-esp32-multicore.png" alt="Perfetto UI Screenshot" />
</p>

## ✨ Key Features

- **Embassy Task Tracing:** See the exact lifecycle and states (Spawned, Running, Waiting, Idle) of all async tasks automatically.
- **Microsecond Precision & Low Overhead:** Uses lock-free ringbuffers per core to gather events with minimal runtime cost.
- **Granular Execution Control:** Easily trace entire functions using `#[monitor_fn]` or zoom into specific code loops with `monitor_scoped!`.
- **Hardware Metrics & Telemetry:** Graph battery voltage, memory usage, or sensor readings right on your timeline using `monitor_value!`.
- **Multi-Core Visualization:** Captures and separates events across multiple CPU cores (e.g., ESP32 App/Pro CPU).
- **Log Correlation:** Your `defmt` messages are displayed on the timeline with millisecond accuracy alongside all other events.

## 📦 What's inside the box?

The project is split into two minimal-overhead components:

1. **`rustmeter-beacon` (The Target):** A lightweight, embedded crate compiled into your firmware. It patches into your executor and ships raw event metrics outward via RTT or UART without blocking your application.
2. **`rustmeter-cli` (The Host):** A CLI tool running on your development machine. It acts as the bridge recording raw data via `probe-rs` or `espflash`, decoding it using your firmware's ELF file, and converting it into a clean JSON trace for analysis.

## 📖 The RustMeter Book

All documentation, including detailed setup instructions, architecture deep-dives, and example guides, is hosted in the official **RustMeter Book**. 

We purposefully keep our READMEs lightweight! For everything you need to get started, head over to the documentation:

👉 **[Read the RustMeter Book](https://christopher-06.github.io/rustmeter/)**

## 💡 A Taste of RustMeter

Imagine you have an I2C sensor task acting up. With RustMeter, you don't need to guess where the time is lost.

```rust
use rustmeter_beacon::{monitor_fn, monitor_value, monitor_scoped};

// 1. Mark your heavy functions to see them on the timeline
#[monitor_fn]
async fn read_sensor(i2c: &mut I2c) {
    let reading = monitor_scoped!("i2c_read", {
        i2c.blocking_read()
    });
    
    // 2. Track the sensor values as a graph
    monitor_value!("temperature", reading.temp);
    
    // defmt logs automatically show up on the timeline!
    defmt::info!("Sensor read complete!");
}
```

Run your firmware using the CLI (`rustmeter run --release --chip esp32`), drop the result into Perfetto, and you immediately see if another task is blocking your I2C job!

## 🤝 Contributing

We welcome contributions! Whether you want to add support for a new MCU architecture, optimize the ringbuffer, or expand the CLI's analysis features. Check out the issues or propose a feature by submitting a PR.

