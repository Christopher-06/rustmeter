# RustMeter Project

RustMeter is a monitoring tool for embedded Rust applications. It consists of a rustmeter-beacon library that can be integrated into embedded Rust projects, and a rustmeter-cli command-line interface for collecting the tracinge data. It uses defmt as the underlying logging framework. 

Afterwards you can visualize the collected data using the Perfetto UI (http://ui.perfetto.dev/), which provides a powerful interface for analyzing performance traces. (See screenshot below) (Perfetto is an open-source project developed by Google for performance tracing and visualization. It does not require any installation as it runs directly in your web browser and does not send any data to external servers, ensuring your data remains private and secure.)

## Features

- Lightweight and efficient monitoring for embedded systems. You can monitor functions calls, execution time and even block specific code sections
- Easy integration with existing Rust projects using the rustmeter-beacon library
- Support Embassy async runtime for asynchronous embedded applications. Showing where tasks are spending and blocking their time
- Your already made DEFMT logs will be collected and shown wihin the monitoring data

## Getting Started

1. Add `rustmeter-beacon` to your embedded Rust project's dependencies:

```sh
cargo add rustmeter-beacon
```

2. (Optional) If you are using the Embassy async runtime, enable the `trace` feature on the embassy-executor:

```toml
[dependencies]
embassy-executor = { version = "x.y.z", features = ["trace"] }
```

and add the rustmeter-beacon code in your main.rs:

```rust
use rustmeter_beacon::*;
```

3. (Optional) Add tracing instruments to your codebase using the provided macros (you can also use scoped monitoring for more granular control e.g. within loops or third-party functions):

```rust
#[monitor_fn]
fn my_complex_function() {
    // Function implementation
}
```

4. (Optional) Add monitoring for custom metrics like sensor readings or other values:

```rust
fn handle_measurement() -> u16 {
    let adc_value: u16 = read_adc();
    event_metric!("adc_read", adc_value);

    // Continue processing
}
```

5. Install the `rustmeter-cli` tool for collecting and analyzing monitoring data:

```sh
cargo install rustmeter
```

6. Run your embedded application with rustmeter to collect monitoring data (optional add the release flag for optimized embedded builds):

```sh
rustmeter 
```

7. Exit the console (Ctrl+C) to stop data collection and view the tracing data via perfetto UI (http://ui.perfetto.dev/). You should see a tracing file like `rustmeter-perfetto.json` in your current directory. 

![Perfetto UI Screenshot](./ressources/perfetto-ui-esp32-multicore.png)


## TODOs

- [ ] Add more examples and documentation
- [ ] Support more embedded platforms and architectures (currently tested on ESP32. Planned: RP2040, STM32)
- [ ] Include RTOS Tracing suport (e.g. ESP-RTOS)
- [ ] Implement advanced filtering and analysis features in rustmeter (CPU usage, memory consumption, etc.)
- [ ] Optimize performance and reduce overhead further
- [ ] Add CI/CD for automated testing of core functionality
- [ ] Create an own book documentation site for better user guidance
