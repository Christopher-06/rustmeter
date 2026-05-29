# Logging with `defmt`

Tracing asynchronous execution traces reveals a tremendous amount of data visually, yet standard text-based logging often supplies critical human context impossible to gauge through strict time graphs alone. 

By brilliantly capturing and routing `defmt` standard logs securely across the very same channel driving trace captures, RustMeter successfully aligns those precious diagnostic context prints precisely against native event timelines reliably inside Perfetto!

### The Reason Behind the Correlation

Unifying your framework logs right onto a singular timeline acts as an exceptional force multiplier for resolving logic issues rapidly! Unlocking log correlation confidently allows you to answer exceptionally obscure situational questions immediately:

- Does this specific `ERROR` printout actually crop up while my networking component is currently holding the CPU, or was my sensor task unfortunately running completely parallel just beforehand?
- How much microscopic delay occurs sitting rigidly between beginning mathematical evaluation sequences sequentially verses when the underlying algorithm ultimately finishes emitting passing warning statuses accurately?
- Exactly what did my tracked board voltage counter visually display squarely before my critical system safety component actively fired panic messages directly into standard logs? 

### ℹ️ Note: Enable the `defmt` Feature in rustmeter-beacon

To ensure your log messages are captured and transmitted correctly, make sure you enable the `defmt` feature in your `rustmeter-beacon` dependency in your `Cargo.toml`:

```toml
[dependencies]
rustmeter-beacon = { version = "...", features = ["defmt", /* your chip */] }
```

This is required for RustMeter to hook into your logging macros and forward them to the host.

### Operating Your Logs Normally

Surprisingly enough, the largest technical hurdle behind integrating log visualization directly via RustMeter consists simply of maintaining your exact typical coding style continuously! 

Apart from importing the standard `defmt` toolsuite natively mapping beside successfully triggering `rustmeter_beacon::init_rustmeter_beacon()`, practically zero explicit framework configuration applies.

```rust
use rustmeter_beacon::monitor_scoped;
use embassy_time::{Duration, Timer};

#[embassy_executor::task]
async fn tracking_my_application() {
    loop {
        // Log exactly as you comfortably do typically!
        defmt::info!("Beginning a fresh polling cycle.");
        
        monitor_scoped!("Critical_Networking_Work", {
            // Intensive logic processing sequences live comfortably here
        });

        if external_safety_alert_triggered() {
            // Warning prompts slip seamlessly onto timeline alignments!
            defmt::warn!("Emergency safety anomaly detected over local threshold!");
        }

        Timer::after(Duration::from_secs(1)).await;
    }
}
```

### Viewing Log Assertions in Perfetto

Rather than endless monochrome text scrolling viciously through a chaotic terminal, every parsed message is intelligently plotted as beautifully defined **instant events** (precise pinpoint markers) situated meticulously onto specific event sequences. These labels retain the original exact chronological timestamp captured explicitly from the CPU dynamically!

Selecting a pin gracefully visualizes corresponding data tags explicitly detailing matching logging severity classes perfectly (like natively resolving `INFO`, `WARN`, or catastrophic `ERROR` categorizations) alongside providing exactly unaltered formatted descriptions cleanly. 

### ⚠️ Crucial Warning: Remove Other Transports

Before diving into the code, it is critical to understand one major constraint: **You must not use `defmt-rtt`, `rtt-target`, or `esp-println` anywhere in your project.**

Because RustMeter automatically takes complete control of the primary data channel (such as RTT or your Serial connection) to quickly stream trace data, including other transport crates will cause them to fight over the exact same connection. This reliably leads to corrupted traces, unreadable logs, or absolute system crashes. 

You should rely *only* on the base `defmt` crate to provide your logging macros. RustMeter will automatically handle the heavy lifting of transporting those logs to your PC!

### Best Practices & Pitfalls to Avoid

- **Bypass Internal Defmt Timestamping Entirely!** Historically, generating hardware-timed timestamp features leveraging `defmt` requires annoying custom configurations alongside timer peripherals carefully orchestrated across embedded boundaries. With RustMeter handling unified time constraints completely transparently, absolutely zero manual `defmt` timestamp logic requires instancing! Lean directly backwards atop our synchronized execution timestamps gracefully.
