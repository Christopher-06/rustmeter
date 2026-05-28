# Troubleshooting

### General Issues & Questions

**Q: I don't see any data in Perfetto, or the file is empty.**

-   **Is your application actually running?** Check if the `rustmeter run` command successfully flashed your device. Look for any error messages during the build or flashing process.
-   **Is the `rustmeter-beacon` initialized?** Ensure that `rustmeter_beacon::init(spawner);` is being called early in your `main` function.
-   **Are you using conflicting crates?** Crates like `rtt-target` or `esp-println` can take exclusive control of the communication channel that RustMeter needs. When tracing, rely only on `defmt` for logging.
-   **Did you instrument anything?** If you disabled the `embassy` `trace` feature and haven't added any manual macros (`#[monitor_fn]`, `monitor_scoped!`), there will be no events to record. Add a simple `#[monitor_fn]` to a task to confirm basic functionality.

**Q: My application crashes or deadlocks when I enable RustMeter.**

-   **Stack Overflow:** Tracing adds a small amount of stack usage to every instrumented function and task. It's possible that you are running out of stack space. Try increasing the stack size for your tasks (e.g., in `embassy-executor`'s `task-arena-size`) or for the entire application in your linker script.
-   **Deadlock with the Logging Task:** The background task that sends trace data needs CPU time to run. If you have a high-priority, busy-looping task that never yields, it can starve the RustMeter task, causing the ring buffer to fill up. In Perfetto you will not see this directly if you then exit the application before the RustMeter task reruns again. Ensure all tasks, especially high-priority ones, have appropriate `await` points (e.g., `Timer::after(...)`).
-   **Deadlock on a single core:** If your code deadlocks and the RustMeter printing task is on the same core, the last events might not be sent. To debug such issues, consider creating a dedicated, high-priority interrupt-based executor just for initializing and running the RustMeter beacon.

### Tracing & Data Issues

**Q: I don't see any Embassy task states (`Running`, `Ready`, `Waiting`).**

-   This is almost always because the `trace` feature flag is missing for the `embassy-executor` crate in your `Cargo.toml`. Double-check it!
    ```toml
    embassy-executor = { version = "...", features = ["...", "trace"] }
    ```

**Q: The names of my functions or logs appear as numbers or are missing.**

-   **Linker Script:** This indicates that the `rustmeter-cli` could not find the necessary metadata in your ELF file. Make sure your `build.rs` correctly includes the `rustmeter.x` linker script (`println!("cargo:rustc-link-arg=-Trustmeter.x");`).
-   **Stripped Binary:** Ensure you are not stripping the ELF file after building it. The CLI needs the symbol information to map IDs back to names.

**Q: The timeline seems to have gaps or stops unexpectedly.**

-   **Buffer Overrun:** This can happen if your application generates events much faster than they can be transmitted to the host. The internal ring buffer might overflow.
    -   **Solution 1:** Reduce the amount of instrumentation. Are you monitoring an extremely high-frequency loop with `monitor_scoped!`? Consider removing it or only enabling it when needed.
    -   **Solution 2 (Advanced):** If using RTT, try increasing the RTT buffer size in your `probe-rs` or flasher configuration.
    -   **Solution 3 (Future):** Increase the size of the ring buffer in the `rustmeter-beacon` configuration. This is a temporary fix and might just delay the problem.

### For Contributors & Advanced Users

**Q: How can I debug the `rustmeter-cli` tool itself?**

-   The CLI is a standard Rust application. You can clone the `rustmeter` repository, navigate to the `rustmeter-cli` directory, and run it with `cargo run -- <COMMAND>`. For example:
    ```shell
    # From within the rustmeter-cli directory
    cargo run -- run --chip RP2040 --release --project /path/to/your/project
    ```
-   Add `println!` or use a debugger like `gdb` or LLDB to inspect its behavior.

**Q: How does the ELF metadata extraction work?**

-   The `rustmeter-beacon` crate does not use any create like `linkme` to create a distributed slice in a special section of the ELF file (e.g., `.rustmeter_meta`).
-   Each macro (`#[monitor_fn]`, `defmt::info!`, etc.) registers a static struct in this section containing a unique ID and the string literal of the name/log. This will get linked into a special elf section and only the address will then get send over the wire. The firmware it self does not have then the string literals in RAM, but only the IDs. This saves a lot of RAM and bandwidth.
-   The `rustmeter-cli` then uses an ELF parsing library to find this custom section, read all the entries, and build a `HashMap<ID, &str>` to map the IDs received from the target back to their human-readable names.

**Q: I want to support a new communication protocol besides RTT and Serial.**

-   The data transmission is abstracted. In `rustmeter-cli`, you would need to implement a new "collector" that knows how to read from your new protocol.
-   In `rustmeter-beacon`, you would need to provide a new implementation for the data sending background task. The core logic of event generation and buffering would remain the same.
-   Other users might be interested in this as well, so consider opening a pull request with your implementation!
- Keep in mind that the communication channel must be able to handle the bandwidth of the trace data, especially if you are generating a lot of events and cannot afford any loss of data OR create to much data while transmitting it. 

**If you have any other issues or questions, please open an issue on the GitHub repository with detailed information about your setup and the problem you are facing.*
