# Tracing Embassy Tasks

One of RustMeter's most attractive highlights is its deep, transparent integration with the `embassy` framework. By turning on a single feature, RustMeter **automatically** begins tracing the lifecycle of your asynchronous tasks without forcing you to litter your elegant code with manual instrumentation. 

This means you can instantly observe when tasks start, when they wake up, when they pause, and crucially, when they find themselves hopelessly waiting.

### How to Enable Task Tracing

Enabling this feature acts as a massive quality-of-life boost for async engineers. 

To activate it, you merely need to expose the `trace` feature flag provided universally down inside the `embassy-executor` dependency itself. Within your `Cargo.toml`, ensure that your dependencies block resembles the example below:

```toml
[dependencies]
embassy-executor = { 
    version = "0.5.0", 
    features = [
        # ... other features you might use like "executor-thread" etc.
        "trace" 
    ] 
}
```

Beyond that flag, you absolutely still must initialize the `rustmeter-beacon` instance during your program's earliest startup routine (usually early inside `main`), exactly as you would have done while reading the Getting Started guide. No extra magical lines are needed per task. 

```rust
// In your main.rs, ensure you load your settings.
use rustmeter_beacon::{init_rustmeter_beacon, RustmeterConfig};
use embassy_executor::Spawner;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Determine your system frequency
    let config = RustmeterConfig::new(120_000_000); 

    // This single robust line unlocks all automatic framework tracing!
    init_rustmeter_beacon(config, &spawner).unwrap();

    // From here, peacefully just spawn your normal tasks
    spawner.spawn(task_one()).unwrap();
    spawner.spawn(task_two()).unwrap();
}
```

### Unveiling The Task Lifecycle

The moment you run your connected chip via `rustmeter run` and load the produced file into Perfetto, you will delightfully be presented with a beautifully organized, dedicated timeline track representing each distinct task spawned. 

RustMeter intrinsically captures the following dynamic statuses across the lifetime of these tasks:

- **Spawned**: At this instantaneous fraction of time, the task was just cleanly created. It signifies the task is completely ready to run, but merely waiting for the CPU to actually get around to executing it.
- **`Running`**: Simply put, the task currently owns your CPU. Code is successfully calculating.
- **`Idle`**: Often the most frequent and healthy async state, this simply denotes that your task has suspended execution awaiting a real-life event, such as an upcoming `Timer`, an ongoing sensor DMA reading, or a filled data channel.
- **`Ready`**: The task successfully received its event and is completely ready to run—but sadly, it sits stuck waiting because your executor chose to process a different active task first. Frequent blocks of `Ready` vividly highlight when your CPU simply has too much to do, pointing directly to potential real-time starvation.
- **`End`**: Peace at last. The task naturally completed computing to the full end and has been peacefully laid to rest.

Suddenly, grasping your software’s concurrency becomes incredibly accessible. You can instantly spot misbehaving routines monopolizing processor time, visibly check if tasks take an unusual duration to resume after waking up, and cleanly prove exactly which task executes at every specific millisecond!

### Visualizing The Executor State

Additionally, RustMeter goes a step further and traces the overarching state of the `embassy-executor` machine itself. 

By observing the timeline track named typically as `embassy-executor`, you gain a profound bird’s-eye perspective depicting when your entire central processor activates versus when it falls comfortably asleep:

- **`Running`**: The broader executor is explicitly running an active task process. You usually even get the visual identifier showcasing strictly which task identifier it is managing.
- **`POLLING`**: The engine is furiously looping through its internal checklists to figure out which waiting routines have recently became ready. This transition logically occurs right after an event executes, or before scheduling kicks off.
- **`Idle`**: Complete silence. No tasks are asking to be run. Embassy seizes this magnificent opportunity to command the whole MCU core to fall right into deep low-power sleep modes, drastically saving your battery capacity dynamically.

### What to Pay Attention To

- **Naming Tasks Automatically:** RustMeter dynamically snags the exact name of the function you wrap and spawn to label the timeline block comfortably. So whenever you call `spawner.spawn(my_cool_task())`, you will see a track confidently labeled `my_cool_task`. It makes navigating chaotic logs visually intuitive.
- **A Missing `trace` Feature:** Should your Perfetto timeline seem oddly devoid of any task event state, the overwhelming reason typically stems from simply forgetting to append that subtle `"trace"` flag onto the `embassy-executor` list within `Cargo.toml`.
- **Free of Manual Macros:** Keep closely in mind you absolutely do **not** need to artificially place the `#[monitor_fn]` macro above your background tasks sequentially just to view this behavior. Everything here is wonderfully autonomous. The only reason to deploy `#[monitor_fn]` over an overarching task is purely if you aim to map its unified total calculation length spanning as a single massive timeline block apart from observing its detailed sleep cycles.
- **Future Multi-Priority Support:** In the future, as we expand to support multi-priority executors in analysis, you may see additional states like `Preempted` in the executor track. For now, you can identify preemption on multi-core systems by correlating if multiple executors are running on the same core simultaneously.