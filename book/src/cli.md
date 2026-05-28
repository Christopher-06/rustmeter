# The rustmeter-cli

The `rustmeter-cli` is the host-side tool that orchestrates the entire tracing process. It's a powerful wrapper that handles building your project, flashing the firmware, capturing the trace data, and converting it into a format that Perfetto can understand.

This guide covers the commands, options, and workflows for using the CLI.

### Commands

The CLI has two main subcommands: `run` and `analyze`.

#### `rustmeter run` (The Default Workflow)

This is the primary command you will use. It performs the full end-to-end process:

1.  **Builds your firmware:** It invokes `cargo build` on your embedded project.
2.  **Flashes the device:** It uses a tool like `probe-rs` or `espflash` to load the compiled ELF file onto your target chip.
3.  **Captures trace data:** It listens for the data stream sent by the `rustmeter-beacon` from your device (typically over RTT or a serial port).
4.  **Analyzes the data:** When you stop the trace (with `Ctrl+C`), it automatically processes the raw data, combines it with information from your ELF file, and generates the final `rustmeter-perfetto.json`.

**Usage:**

```shell
rustmeter run [OPTIONS]
```

**Options for `run`:**

-   `--chip <CHIP_NAME>`: **(Required)** Specifies the target microcontroller. This is crucial for the flasher to know how to communicate with the device.
    -   Examples: `--chip RP2040`, `--chip STM32F446RE`, `--chip ESP32-S3`.

-   `--release`: Builds the project in release mode (`cargo build --release`). Highly recommended for embedded systems and more accurate performance measurements.

-   `--project <PATH>`: Specifies the path to your embedded project directory. By default, it uses the current directory (`.`).

-   `--tool <TOOL_NAME>`: Manually select the flashing and monitoring tool.
    -   `probe-rs`: Use `probe-rs`. This is the default for most non-Espressif chips and uses RTT for communication.
    -   `espflash`: Use `espflash`. This is the default for all Espressif chips and uses the serial port.
    -   `auto`: (Default) Let RustMeter choose the best tool based on your `--chip`.

-   `<EXECUTABLE>`: Instead of building, you can provide a path to a pre-compiled ELF file. This is useful for re-analyzing a specific build without recompiling. When you use this, `--release` and `--project` are ignored.

#### `rustmeter analyze`

This command skips the build and run steps and just performs the analysis on an existing set of trace files.

**Usage:**

```shell
rustmeter analyze --folder <PATH_TO_TRACING_FOLDER>
```

-   **Why use this?** The `rustmeter run` command saves the raw, unprocessed trace data in a folder (usually `target/rustmeter/tracing/<timestamp>`). If you want to re-run the analysis with a newer version of the analyzer or different settings in the future, you can point the `analyze` command at this folder.

### Common Workflows & Questions

**Q: How do I run my project for tracing?**

A: The simplest way is to navigate to your project's root directory and run:

```shell
rustmeter run --chip <YOUR_CHIP> --release
```

**Q: Can I still use `cargo run`?**

A: Yes! You can configure `rustmeter` as your default cargo runner. This is very convenient.

1.  Create a file named `.cargo/config.toml` in your project root.
2.  Add the following content, adjusting the target and chip name:

    ```toml
    [target.thumbv7em-none-eabihf] # <-- Change to your target
    runner = "rustmeter run --chip STM32F746ZGT6" # <-- Change to your chip

    [build]
    target = "thumbv7em-none-eabihf" # <-- Change to your target
    ```

3.  Now, you can simply run `cargo run --release` as you normally would, and it will automatically use `rustmeter` to flash and trace your application.

**Q: Where is the output file?**

A: The final trace file, `rustmeter-perfetto.json`, is saved in the root of the directory where you ran the `rustmeter` command.

**Q: What does the CLI do with the ELF file?**

A: The ELF file is essential. The `rustmeter-beacon` on your device only sends small, numerical IDs for function names and log strings to save bandwidth. The `rustmeter-cli` reads the ELF file on your PC to map these IDs back to their full names, which are then displayed in Perfetto. This is why the linker script setup is so important.
