# Monitoring Functions and Scopes

While letting the automatic task tracing engine handle background embassy events is marvelously powerful, you frequently will encounter scenarios where you demand granular, laser-focused control to isolate very specific bottlenecks buried deep within your logic. 

To easily accommodate this, RustMeter graciously ships several easy-to-use macros that allow you to manually segment, tag, and measure exact snippets of computational work. 

### `#[monitor_fn]`: Monitoring Entire Functions

The `#[monitor_fn]` attribute securely attaches to any function definition. Once applied, it automatically times the length of processing starting the exact moment you enter the function extending precisely until it reaches the exiting return.

For standard synchronous software architecture (`fn`), it works wonderfully straightforward. It maps the overall time spent strictly inside that dedicated block spanning end-to-end within Perfetto:

```rust
use rustmeter_beacon::monitor_fn;

#[monitor_fn]
fn my_heavy_blocking_calculation() {
    // An intensive calculation lives here.
    // RustMeter will illustrate the full duration effectively as one massive block.
}
```

Interestingly, things become profoundly dynamic once you attach it to an `async fn`. RustMeter ingeniously breaks apart the life of an asynchronous function actively according to the location of its `.await` points!

Every fragment of executing code situated neatly between individual `.await` interruptions is treated dynamically as a separate `Running` event inside the timeline. For instance, rather than showing a long two-minute pause, you accurately witness exactly which phase required computing and identifying just how massive a time window was spent effectively waiting. 

```rust
use rustmeter_beacon::monitor_fn;

#[monitor_fn]
async fn my_async_operation() {
    // This top portion triggers the first visible execution block.
    let data = fetch_data().await; 

    // Once woken up, this chunk creates the second execution block.
    process_data(data).await;

    // Finally, closing instructions log the third block seamlessly.
}
```

By default, RustMeter beautifully mirrors the exact rust function name you assigned the routine. Nevertheless, should you prefer a specialized explicit label visually, you can easily append a custom name parameter directly onto the macro mapping itself!

```rust
#[monitor_fn(name = "MyAwesomeCustomName")]
fn internal_utility_function() {
    // The timeline track shows "MyAwesomeCustomName" directly.
}
```

### `step!`: Marking Points in Async Functions

Within those complex async routines wrapped carefully inside `#[monitor_fn]`, it can sometimes become rather tedious tracking which `.await` phase you are looking at spanning alongside massive traces. The helpful `step!` macro allows you to drop descriptive checkpoints across these segments freely. 

Simply call `step!("your explicit step name")` and watch the label beautifully manifest against subsequent `Running` execution tracks inside Perfetto!

```rust
use rustmeter_beacon::{monitor_fn, step};

#[monitor_fn]
async fn massive_configuration_task() {
    step!("Starting the network phase");
    // Work done here will inherit the updated label inside Perfetto.
    connect_to_wifi().await;

    step!("Downloading configuration payload");
    // Work done here appropriately updates and registers against this new label.
    pull_http_payload().await;
    
    step!("Processing constraints locally");
    // Followed by closing logic.
}
```

*Word of Caution:* Keep your broader executing model in mind. Dropping a `step!` call casually situated inside an aggressively looping `for` iteration forces the step label openly against subsequent `.await` phases outside the scope if unchallenged directly.

### `monitor_scoped!`: Profiling Arbitrary Code Blocks

Routinely, you don't actually intend tracking entirely massive functions top-to-bottom. Sometimes you simply want to analyze an obscure calculation hidden squarely inside a larger functional machine. This is perfectly where `monitor_scoped!` shines!

Take any bracket of routine code lines and comfortably embrace them together utilizing `monitor_scoped!("my_special_scope_name", { ... });`. 

This shines particularly bright when you aim to dissect the length of looping arrays or profiling an edge-case calculation embedded inside standard algorithms:

```rust
use rustmeter_beacon::monitor_scoped;

fn generic_system_controller() {
    // Surrounding business logic occurs securely.

    monitor_scoped!("CriticalSensorLoop", {
        for _ in 0..10_000 {
            // Intensive isolated processing sits comfortably here...
        }
    });

    // Remainder of your system flows freely...
}
```

**Can I run `step!` inside `monitor_scoped!`?** 
Actually, no. The elegant `step!` macro is architectured expressly to integrate securely with the native state machine found natively on an authentic `async fn` labeled forcefully by `#[monitor_fn]`. It won't yield meaningful visual markers placed aimlessly inside a `monitor_scoped!` section. 

However, you can brilliantly nest a `monitor_scoped!` segment firmly *inside* an `async fn` properly decorated with `#[monitor_fn]`! The timeline actively builds a stunning hierarchy showing exactly your custom scope duration alongside stepping phases seamlessly!
