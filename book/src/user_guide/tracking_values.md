# Tracking Values

Besides strictly capturing timing events, developing a thorough understanding of an embedded system regularly requires visualizing exactly how specific system values wildly fluctuate over your run intervals. The `monitor_value!` macro was deliberately built for precisely this scenario, natively bridging variable tracking efficiently onto your main visual map.

### Why Target Value Tracking?

Graphing runtime fluctuations proves incredibly rewarding for uncovering oddities visually. Use it whenever you depend on understanding dynamically altering metrics across your product:

- **Sensor Data Analytics:** Graph live temperature dips, pressure thresholds, damp accelerometers responses, or I2C sensor swings correctly aligned against your execution steps.
- **Underlying System State:** Watch your available free memory drain consistently, monitor volatile battery lines under active loads, or pinpoint exactly when computational thresholds peak.
- **Application Level Tracking:** Effortlessly graph your message queue lengths filling up across peaks, record exactly how many dropped packages accumulate visually, or just cleanly observe loop iterations natively.

When generated, Perfetto eagerly translates these numeric snapshots elegantly into **Counter Tracks**. These manifest as gorgeous continuous graphs mapping tracked values tightly along the vertical Y-axis plotted directly against your timeline’s exact chronological X-axis!

### Applying the Macro Practically

Utilizing the macro sits elegantly along the natural feel of Rust logging. Pass it two clear arguments: simply a string-literal specifying your chosen metric name, immediately followed closely by the explicit numeric value you aim to register at that specific passing moment.

```rust
use rustmeter_beacon::monitor_value;

// Recording a raw literal integer against a specific visual track!
monitor_value!("metric_board_voltage", 3300);
```

The system beautifully supports mapping classic `u32`, `i32`, `u64`, `i64`, and natively plots decimal floating points securely utilizing `f32` and `f64` values safely.

**Example: Plotting Active Memory**

Suppose you actively wield a local function measuring your board's remaining available bytes `get_free_heap_size()`. Constructing a simple background task querying this consistently crafts a perfect memory graph perfectly:

```rust
use rustmeter_beacon::monitor_value;
use embassy_time::{Duration, Timer};

#[embassy_executor::task]
async fn memory_monitor_task() {
    loop {
        // Collect local state from your application
        let free_memory = get_free_heap_size(); 
        
        // Push the recorded value directly up into the timeline
        monitor_value!("free_heap", free_memory as u32);
        
        // Wait gently beforehand polling the system again.
        Timer::after(Duration::from_secs(1)).await;
    }
}
```

By safely running `spawner.spawn(memory_monitor_task())` directly alongside your main firmware loops, a crisp graph clearly named `free_heap` manifests completely visually isolating memory leak behaviors precisely!

### Important Usage Tips

- **Naming Enforces Unification:** Perfetto associates graph paths utilizing matching string-literal titles. If you systematically submit values marked constantly as `"free_heap"`, RustMeter plots those updates smoothly plotting adjacent segments on the *same exact* graph layer line. If you strictly need separating elements natively, simply deploy notably disparate identifying titles!
  
  ```rust
  // Generates explicitly disconnected counter graphs alongside your timeline naturally.
  monitor_value!("temperature", 25.5);
  monitor_value!("humidity", 45.2); 
  ```

- **Consistent Types Matter:** While RustMeter politely accepts varied numeric primitive types against a single timeline cleanly, heavily altering types passed into a matching metric name can skew visual scaling uncomfortably within Perfetto. Standardize a single type when continually reporting targeting identical named metrics!
- **Slight Overhead Cautions:** As similarly documented spanning measuring traits, forcefully logging dynamic integers triggers microscopic processing footprint constraints dynamically. Be somewhat careful repeatedly triggering tracking loops hundreds of thousands of times within performance-critical interrupts specifically. Focus your tracks natively where metric updates logically alter substantially.
