# The `rustmeter-cli`

The `rustmeter-cli` is the command-line tool installed on your host PC, and it orchestrates the entire tracing process. You can think of it as a powerful wrapper that takes care of compiling your project, flashing your microcontroller, capturing the outgoing data stream via RTT or a serial port, and finally transforming it all into the standardized format that the Perfetto UI understands.

In this chapter, we will explain the core commands you use to interact with the CLI and showcase common workflows.

### Commands

The CLI primarily operates using two subcommands: `run` and `analyze`.

#### `rustmeter run` (The Default Workflow)

This is the command you will use most often. It takes care of the entire end-to-end continuous loop from code to trace visualizer:

1. **Builds your firmware**: First, it invokes `cargo build` on your embedded project.
2. **Flashes the device**: Next, it utilizes tools like `probe-rs` or `espflash` to write the newly compiled ELF binary onto your target.
3. **Captures trace data**: Once the device starts running, it persistently listens for the event stream sent by the `rustmeter-beacon` running on your chip.
4. **Analyzes the data**: The moment you end the session by pressing `Ctrl+C`, the CLI processes the raw incoming data. It efficiently references your local ELF file and ultimately saves the result as `rustmeter-perfetto.json`.

**Usage:**

```shell
rustmeter run [OPTIONS] [EXECUTABLE]
```

**Options for `run`:**

- `--chip <CHIP_NAME>`: **(Required)** This tells the CLI which specific microcontroller model you are aiming to trace. The flasher needs this accurate name to establish a connection. Examples include `--chip RP2040` or `--chip ESP32-S3`.
- `--release`: This tells the tool to compile the project in release mode (`cargo build --release`). We highly recommend doing this since optimizations make performance measurements much more reliable and representative of your real-world application.
- `--project <PATH>`: With this option, you can specify exactly where your embedded project's root folder is located. By default, it operates in the current directory (`.`).
- `--tool <TOOL_NAME>`: By default, RustMeter tries to guess the flashing tool. However, you can manually dictate which tool handles flashing and monitoring:
  - `probe-rs`: Use `probe-rs`, which uses RTT for communication and is the standard for non-Espressif controllers.
  - `espflash`: The default standard for Espressif chips that heavily use serial port communication.
  - `auto`: (Default) Let RustMeter use heuristics based on your `--chip` setting to pick the correct background tool.

Sometimes you don't even want to rebuild from scratch but start right from a pre-compiled `.elf` executable. You can provide its path directly as the final argument (e.g. `rustmeter run --chip RP2040 ./target/thumbv.../release/my_binary`). **Please note**: Giving an `<EXECUTABLE>` conflicts directly with `--release` and `--project`, making them unavailable for that launch.

#### `rustmeter analyze`

If you ever wish to skip the compiling and flashing stages entirely—perhaps to rerun an analysis step on historical trace files—you can reach for the `analyze` subcommand.

**Usage:**

```shell
rustmeter analyze [PATH_TO_TRACING_FOLDER]
```

- **Why use this?** Every time you utilize `rustmeter run`, the underlying raw raw trace streams are preserved quietly inside your build folder (typically somewhere like `target/rustmeter/tracing/<timestamp>`). Should you wish to re-evaluate those traces (for instance, to experiment with a newer analyzer algorithm), you can command the `analyze` action directly to interpret that particular folder. If you omit the folder path, it defaults to the current working directory `.`.

### Common Workflows & Questions

**How do I run my project for tracing normally?**

The clearest approach involves opening a terminal in your project's root folder and launching:

```shell
rustmeter run --chip <YOUR_CHIP> --release
```

**Can I still use `cargo run`?**

Absolutely! Integrating `rustmeter` to impersonate your default cargo runner is delightfully convenient. In your project, locate or create a `.cargo/config.toml` file at the root, and configure it similarly to the snippet below. Be sure to substitute your correct target architecture and chip:

```toml
[target.thumbv7em-none-eabihf] # <-- Substitute your specific build target
runner = "rustmeter run --chip STM32F746ZGT6" # <-- Make sure to declare your exact chip

[build]
target = "thumbv7em-none-eabihf" # <-- Similarly, specify your target here
```

Now, every time you execute `cargo run --release`, Cargo natively delegates the final step directly to `rustmeter`, which gracefully compiles, flashes, and jumps into trace-listening mode.

**Where is the output file stored?**

You will find the generated timeline trace file, `rustmeter-perfetto.json`, happily sitting in the directory from which you invoked the command. 

**What does the CLI do with the ELF file?**

The host PC's locally stored ELF file serves a profound purpose. The `rustmeter-beacon` running on your device strictly sends over lightweight numerical IDs, saving precious network bandwidth entirely by skipping raw string text. It is the job of the `rustmeter-cli` analyzer to meticulously map these numerical IDs back into their original, human-readable function and log names found hidden inside the ELF file. This maps cleanly into Perfetto, making custom linker script integration deeply essential.
