# RustMeter Beacon

**The embedded instrumentation library for the RustMeter profiling system.**

[`rustmeter-beacon`](https://crates.io/crates/rustmeter-beacon) is a lightweight tracing library designed for embedded Rust applications. It serves as the device-side component that captures runtime events, performance metrics, and task transitions.

It is built to integrate seamlessly with the [Embassy](https://github.com/embassy-rs/embassy) async executor and uses [defmt](https://github.com/knurling-rs/defmt) for highly efficient, low-overhead logging.

## ✨ Features

- **Embassy Integration**: Hooks into `embassy-executor` to trace task states (Spawn, Run, Wait, Idle).
- **Function Monitoring**: Easily instrument critical functions with the `#[monitor_fn]` attribute.
- **Scoped Tracing**: Measure execution time of specific blocks or loops with `monitor_scoped!`.
- **Custom Metrics**: Log sensor data or internal state variables to visualize them over time.
- **Multi-Core Ready**: Identifies which core code is running on (currently supports ESP32 Xtensa & RISC-V).

## 📦 Installation

Add the crate to your embedded project's `Cargo.toml`.

**Note:** To enable embassy task tracing, you **must** also enable the `trace` feature of the `embassy-executor`.

**Attention:** Defmt must be configured properly with **`timestamp`** enabled (See [defmt documentation](https://defmt.ferrous-systems.com/timestamps) for more details). Else, the tracing will not work correctly because of missing timestamps.

```toml
[dependencies]
rustmeter-beacon = "X"
defmt = "X"

# Important: Enable the 'trace' feature!
embassy-executor = { version = "X", features = ["trace", ... ] }
```

## 🛠️ Usage

1. Setup

Simply import the crate in your application entry point. This ensures the linker includes the necessary trace hooks.

```rust
use rustmeter_beacon::*;
```

2. Instrument Functions

Use the #[monitor_fn] attribute to trace the start and end of a function. This works for both async and sync functions.

```rust
#[monitor_fn]
async fn process_data() {
    // ... heavy lifting
}

#[monitor_fn("MyCustomLabel")] // Override the name shown in the trace
fn interrupt_handler() {
    // ...
}
```

3. Trace Scopes

For more granular control, use the monitor_scoped! macro to measure specific code blocks.

```rust
fn complicated_calculation() {
    // ... setup

    let result = monitor_scoped!("FFT_Calculation", {
        // This block will be timed separately
        perform_fft(&data)
    });

    // ... teardown
}
```

4. Record Metrics

Visualize values over time (like battery voltage, memory usage, or temperature) using event_metric!. These appear as counter graphs in the trace viewer.

```rust
let temp = temp_sensor.read();
event_metric!("temperature", temp);
```

## 🚀 Collecting Data

This library produces `defmt` logs. To capture and visualize them, you need the host-side tool `rustmeter`.

1. Install the CLI: `cargo install rustmeter`

2. Run your project: `rustmeter` (instead of cargo run)

3. Open the generated trace file in [ui.perfetto.dev](https://ui.perfetto.dev/).

## ⚙️ Architecture Support

Currently, rustmeter-beacon includes automatic core ID detection for:

- Espressif ESP32 (Xtensa & RISC-V variants via esp-hal)

Support for other platforms (e.g., STM32, RP2040) is planned.

## 📄 License

This project is licensed under the MIT License.
