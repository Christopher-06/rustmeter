# RustMeter CLI

**The host-side companion for the RustMeter embedded profiling system.**

[**RustMeter**](https://christopher-06.github.io/rustmeter/) is a comprehensive profiling, tracing, and monitoring toolkit designed specifically for **Embedded Rust** applications. It visualizes the execution of your microcontroller in the [Perfetto UI](https://ui.perfetto.dev/), providing deep insights into task scheduling and firmware performance.

<p align="center">
  <img src="https://raw.githubusercontent.com/Christopher-06/rustmeter/refs/heads/main/ressources/perfetto-ui-esp32-multicore.png" alt="Perfetto UI Screenshot" />
</p>

## ✨ What does the CLI do?

The `rustmeter-cli` runs on your developer PC. Its job is to capture the highly-compressed raw data stream originating from your microcontroller (sent over RTT or UART by `rustmeter-beacon`). 

The CLI tool:
- Coordinates the flash & run process via tools like `probe-rs` or `espflash`.
- Records incoming trace bytes on the fly.
- Decodes the raw binary data using metadata extracted directly from your compiled ELF firmware.
- Generates standardized JSON trace files ready for drag-and-drop into Perfetto.

## 📖 Get Started

We keep installation and setup instructions out of the README so they never fall out of sync. For a full guide on installing the CLI and configuring your project runners, please read the official documentation:

👉 **[RustMeter Book: Getting Started](https://christopher-06.github.io/rustmeter/)**

