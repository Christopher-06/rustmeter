# Getting Started

Let’s walk through the steps to set up this powerful tool in your embedded Rust project. We will guide you over installing the host tools, configuring your embedded application, and viewing your first trace. Don't worry—the setup is designed to be straightforward.

## Installation

Before you begin, make sure your embedded Rust development environment is ready. You will need `rustup`, `cargo`, and the appropriate compilation target for your microcontroller (for example, `thumbv7em-none-eabi`). Additionally, you should have a debug probe set up with `probe-rs` or an active USB connection for Espressif chips.

### Step 1: Install `rustmeter-cli` on Your Host PC

The `rustmeter-cli` is the command-line tool that acts as your command center. It builds and flashes your project, listens for incoming trace data, and prepares it for visualization in Perfetto.

Open your terminal and run the following command to install it globally:

```shell
cargo install rustmeter-cli
```

Once the installation finishes, you can verify that it is available on your system by checking the version:

```shell
rustmeter --version
```

### Step 2: Add `rustmeter-beacon` to Your Embedded Project

Next, you need to add the `rustmeter-beacon` library to your embedded firmware. This library handles all the data collection and transmission on the microcontroller side.

Open your project's `Cargo.toml` file and add the following line to your `[dependencies]` section. Make sure to choose the correct chip-specific feature that matches your board!

```toml
[dependencies]
rustmeter-beacon = { version = "*", features = ["YOUR_CHIP_HERE", "defmt"] }
```

You can replace `"YOUR_CHIP_HERE"` with the feature flag corresponding to your microcontroller. The currently supported chip features are:
- `stm32`
- `esp32`
- `esp32c2`
- `esp32c3`
- `esp32c6`
- `esp32h2`
- `esp32s2`
- `esp32s3`
- `rp2040`

**Crucial for Embassy Users:** If you want RustMeter to automatically trace your asynchronous task lifecycles, you also need to enable the `trace` feature for the `embassy-executor` crate in your `Cargo.toml`.

```toml
embassy-executor = { version = "*", features = [ /* ... */, "trace"] }
```

### Step 3: Configure the Linker Script

RustMeter optimizes performance by using a custom linker script. Instead of continuously sending the names of your functions and tasks over the wire (which wastes bandwidth and memory), it stores them directly in the final ELF file on your PC.

To instruct `cargo` to use this script, you must add it to your build configuration. If you don't already have a `build.rs` file in your project's root directory, create one now and add the following lines:

```rust
fn main() {
    // Add the RustMeter linker script
    println!("cargo:rustc-link-arg=-Trustmeter.x");
}
```

If your project uses other linker scripts (such as `defmt.x` or `memory.x`), make sure `rustmeter.x` is included alongside them. The order of linker scripts generally matters, so adding it alongside your existing `println!` statements is typically a safe approach.

## Your First Trace

Now that everything is installed and configured, let's actually trace something!

### Step 1: Initialize the Beacon in Your Firmware

In your `main.rs`, you need to initialize the beacon. This starts the background process that transmits trace data to your host PC. Because `rustmeter-beacon` requires accurate timekeeping, you must provide your system's clock frequency in Hertz.

Here is a minimal example using `embassy`:

```rust
use rustmeter_beacon::{init_rustmeter_beacon, RustmeterConfig, get_system_freq};
use embassy_executor::Spawner;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // ... initialize your hal and clock ...

    // Initialize RustMeter beacon with the correct clock frequency
    let config = RustmeterConfig::new(get_system_freq!());
    init_rustmeter_beacon(config, &spawner).expect("Failed to initialize RustMeter");

    // ... continue with your main application logic
}
```

### Step 2: Instrument Your Code

To see activity on your timeline, tell RustMeter what you want to measure. The easiest way to do this is by adding the `#[monitor_fn]` attribute to a function.

Note: Embassy Tasks will be automatically traced if you enabled the `trace` feature for `embassy-executor`, so you don't need to add any manual instrumentation to them. However, for other functions or specific code blocks, you can use the provided macros later in the guide.

```rust
use rustmeter_beacon::monitor_fn;
use embassy_time::{Duration, Timer};

#[monitor_fn]
async fn my_monitored_time_spending() {
    // The total time spent in this function will be recorded
    Timer::after(Duration::from_millis(100)).await;
}
```

### Step 3: Run the Application with `rustmeter-cli`

Instead of typing `cargo run`, you will now launch your application through `rustmeter run`. The CLI needs to know which chip you are targeting so it can invoke the proper flashing routine.

Open your terminal and execute the following command, ensuring you replace `<YOUR_CHIP>` with your specific model (e.g., `RP2040` or `ESP32-S3`):

```shell
rustmeter run --chip <YOUR_CHIP> --release
```

The CLI will build your code, flash your microcontroller, run the application, and start listening for the incoming data stream automatically.

### Step 4: Stop Tracing and View the Results

Let your application run for a few seconds so it can gather some events. Once you forms are done, gently press `Ctrl+C` in your terminal to stop the process.

RustMeter will finalize the capture and generate a JSON file in your project folder. You can drag and drop this file right into the [Perfetto UI](https://ui.perfetto.dev/) to interactively explore your very first system trace!

- The CLI will stop the trace and save the output to a file named `rustmeter-perfetto.json`.
- Go to [ui.perfetto.dev](https://ui.perfetto.dev/).
- Click on "Open trace file" and select the `rustmeter-perfetto.json` file generated in your project's root directory.
- You should now see a timeline visualizing the execution of `my_monitored_time_spending`!

### Recommended Workflow: Using the Cargo Runner

While calling `rustmeter run` directly works well, the recommended approach is to integrate it into your standard `cargo run` workflow. This is more convenient and feels more natural.

-   **How:** Create a `.cargo/config.toml` file in your project's root directory.
-   **What to add:** Configure `rustmeter run` as the "runner" for your specific target.

    ```toml
    # in .cargo/config.toml

    [target.thumbv7em-none-eabihf] # <-- IMPORTANT: Change this to your actual target triple
    runner = "rustmeter run --chip STM32F446RE" # <-- IMPORTANT: Change this to your chip

    # Optional, but recommended:
    [build]
    target = "thumbv7em-none-eabihf" # <-- Also change this to your target
    ```

-   **The Benefit:** Now, you can simply run `cargo run` and Cargo will automatically use `rustmeter` to flash and trace your application.

### Always Use `--release` for Profiling!

When you are measuring performance, it is crucial to build your code in **release mode**. Debug builds are not optimized and contain extra checks, which makes any timing measurements inaccurate.

-   **With `rustmeter run`:**
    ```shell
    rustmeter run --chip <YOUR_CHIP> --release
    ```

-   **With the Cargo runner workflow:**
    ```shell
    cargo run --release
    ```

This ensures that you are profiling the optimized version of your code, giving you a true picture of your application's performance. This is also the default mode for embedded development, so it's generally best practice to always use release mode when running on actual hardware.
