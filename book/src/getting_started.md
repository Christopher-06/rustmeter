# Getting Started

## Installation

- **Prerequisites:**
    - Assumes you have a working Rust embedded development environment set up (including `rustup`, `cargo`, and the correct target for your microcontroller, e.g., `thumbv7em-none-eabi`).
    - You need a debug probe supported by `probe-rs` (like a J-Link, ST-Link, or the integrated probe on many dev boards) to flash the firmware and get the RTT data. For Espressif chips, a simple USB connection is sufficient.

- **Step 1: Install the `rustmeter-cli` on your Host PC**
    - This is the command-line tool that will build and run your project, receive the trace data, and convert it for Perfetto.
    - Open your terminal and run the following command:
        ```shell
        cargo install rustmeter-cli
        ```
    - Verify the installation by checking the version:
        ```shell
        rustmeter --version
        ```

- **Step 2: Add `rustmeter-beacon` to your Embedded Project**
    - This is the library that lives in your firmware to collect and send the data.
    - Open your project's `Cargo.toml` file.
    - Add `rustmeter-beacon` to your `[dependencies]`:
        ```toml
        [dependencies]
        rustmeter-beacon = { version = "0.1", features = ["YOUR_CHIP_XXX", "defmt"] }
        ```
        TODO: Add Chip features here as a list which are supported
    - **Crucial for Embassy Users:** You also need to enable the `trace` feature for `embassy-executor` to allow RustMeter to automatically trace task events.
        ```toml
        # Example for embassy-executor
        embassy-executor = { version = "0.5.0", features = [..., "trace"] }
        ```

- **Step 3: Configure the Linker Script**
    - RustMeter uses a custom linker script to efficiently store the names of your monitored functions and tasks directly in the ELF file on your host PC, instead of sending them over the wire. This saves a significant amount of bandwidth and memory on the target device.
    - It also enables advanced features for tracing asynchronous functions.
    - You need to tell `cargo` to use this script. Create or modify the `build.rs` file in your project's root directory (next to `Cargo.toml`) if you don't have one already.
    - Add the following line to your `build.rs`:
        ```rust
        fn main() {
            // ... other build script logic

            // Add the rustmeter linker script
            println!("cargo:rustc-link-arg=-Trustmeter.x");
        }
        ```
    - **Note:** The order of linker scripts can be important. If you are using other scripts like `defmt.x` or `memory.x`, ensure `rustmeter.x` is included. A common setup in `build.rs` might look like this:
        ```rust
        fn main() {
            println!("cargo:rustc-link-arg=-Tdefmt.x");
            println!("cargo:rustc-link-arg=-Trustmeter.x");
            // for some targets, a final script is needed
            println!("cargo:rustc-link-arg=-Tlinkall.x");
        }
        ```

## Your First Trace

- **Step 1: Initialize the Beacon in your Firmware**
    - In your `main.rs`, you need to initialize the `rustmeter-beacon`. This sets up the background task that sends data to the host.
    - It's best to do this early in your `main` function.
    - Here is a minimal example:
        ```rust
        use rustmeter_beacon::prelude::*;

        #[main]
        async fn main(spawner: Spawner) {
            // ... your other initializations
            
            // Initialize RustMeter beacon
            rustmeter_beacon::init(spawner);

            // ... rest of your app
        }
        ```

- **Step 2: Instrument your Code**
    - To see something in your trace, you need to tell RustMeter what to monitor. The easiest way is to monitor a function.
    - Find a function in your code (or create a simple one) and add the `#[monitor_fn]` attribute.
        ```rust
        #[monitor_fn]
        async fn my_monitored_task() {
            // ... some work ...
            Timer::after(Duration::from_millis(100)).await;
        }
        ```

- **Step 3: Run the Application with `rustmeter-cli`**
    - Instead of `cargo run`, you will now use `rustmeter run`.
    - The CLI needs to know which chip you are using to correctly flash and connect to it.
    - Run the following in your terminal, replacing `<YOUR_CHIP>` with your target (e.g., `STM32F446RE`, `RP2040`, `ESP32-S3`).
        ```shell
        rustmeter run --chip <YOUR_CHIP>
        ```
    - The application will build, flash, and run. The `rustmeter-cli` will automatically start listening for trace data.

- **Step 4: Stop Tracing and View the Results**
    - Let the application run for a few seconds.
    - Press `Ctrl+C` in the terminal where `rustmeter` is running.
    - The CLI will stop the trace and save the output to a file named `rustmeter-perfetto.json`.
    - Go to [ui.perfetto.dev](https://ui.perfetto.dev/).
    - Click on "Open trace file" and select the `rustmeter-perfetto.json` file generated in your project's root directory.
    - You should now see a timeline visualizing the execution of `my_monitored_task`!

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
