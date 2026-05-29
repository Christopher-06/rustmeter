# RustMeter Beacon Target

**Target-specific hardware implementations for [RustMeter Beacon](https://crates.io/crates/rustmeter-beacon).**

This internal crate contains all architecture-specific communication backends (e.g., Espressif, STM32, RP2040) ensuring the raw trace data makes its way out of the microcontroller efficiently. **It is not meant to be used directly by end-users.**

Please use the main `rustmeter-beacon` crate to instrument your applications.

👉 **[Learn more in the RustMeter Book](https://christopher-06.github.io/rustmeter/)**
