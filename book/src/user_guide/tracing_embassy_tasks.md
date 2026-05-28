# Tracing Embassy Tasks

One of RustMeter's most powerful features is its deep integration with the `embassy` framework. Once enabled, it **automatically** traces the lifecycle of all your asynchronous tasks without requiring you to add any manual instrumentation to them.

### How to Enable It

- **Requirement:** This feature is only active if you enable the `trace` feature flag for the `embassy-executor` crate in your `Cargo.toml`.

- **Example `Cargo.toml`:**
  Make sure your `embassy-executor` dependency includes `"trace"`.

  ```toml
  [dependencies]
  embassy-executor = { 
      version = "0.5.0", 
      features = [
          # ... other features like "executor-thread" etc.
          "trace" 
      ] 
  }
  ```

- **Initialization:** You still need to initialize the `rustmeter-beacon` in your `main` function as shown in the "Getting Started" guide.

  ```rust
  // in your main.rs
  use rustmeter_beacon::prelude::*;
  use embassy_executor::Spawner;

  #[main]
  async fn main(spawner: Spawner) {
      // ... other initializations
      
      // This single line enables all automatic tracing
      rustmeter_beacon::init(spawner);

      // Now, just spawn your tasks as usual
      spawner.spawn(task_one()).unwrap();
      spawner.spawn(task_two()).unwrap();
  }
  ```

### What You Get: The Task Lifecycle

Once you run your application with `rustmeter run`, you will see a dedicated track for each spawned task in the Perfetto UI. RustMeter visualizes the following states for each task:

- **Spawned**: The task has been created and is ready to run but has not yet started executing. This should only be a very brief state before it transitions to `Running` or `IDLE` at system startup (or when the task is first spawned).
-   **`Running`**: The task is actively executing code on the CPU.
-   **`IDLE`**: The task is waiting for the event to occur (e.g., waiting for a `Timer`, a channel to receive data, or a `Future` to complete). This is the most common state for well-behaved async tasks.
-   **`WAITING`**: The task is ready (event occured) but is not currently running because the executor has chosen to run another task. This can indicate that the task is being starved of CPU time or that there are higher-priority executors running.
- **`END`**: The task has completed its execution and is no longer scheduled to run.

This gives you an immediate and clear overview of your application's concurrency:

-   Which tasks are running at any given moment?
-   How much time do tasks spend waiting vs. running?
-   Are there any tasks that are unexpectedly blocked or not running at all?

### What You Get Part 2: The Executor State

In addition to tracing individual tasks, RustMeter also visualizes the state of the `embassy-executor` itself. This provides a high-level view of your CPU's activity and is crucial for understanding overall system load and power consumption.

You will see a track in Perfetto, typically named `embassy-executor`, showing the following states:

-   **`Running`**: The executor is busy running a task. The task id will get displayed.
-  **`POLLING`**: The executor is actively checking for tasks that are ready to run. This can happen when there are tasks in the `Waiting` state that have just become `Ready` or any Task was ready and now the executor is deciding which one to run next (or got to IDLE).
-   **`Idle`**: The executor has no tasks that are `Ready` to run. This happens when all spawned tasks are in the `Waiting` state. In this state, Embassy can put the CPU into a low-power sleep mode, which is a key feature for battery-powered devices (if all executors are idle, the CPU can sleep). 

In the future you may also see states like `Preempted` in multi-prio systems. This means that by now, you will not see when an Executor gets preempted. Instead you should correlate if on the same core two executors are running at the same time, which means any of them got preempted.

*(Here you could add a screenshot of the Perfetto UI showing the task and executor tracks together)*

### What to Pay Attention To

-   **Task Naming:** The name of the task track in Perfetto is automatically derived from the name of the function that you spawn. For example, `spawner.spawn(my_cool_task())` will create a track named `my_cool_task`. This makes it easy to identify what's happening.

-   **The `trace` Feature is Crucial:** If you don't see any task events in your trace, the most common reason is that you forgot to enable the `trace` feature on `embassy-executor`. Double-check your `Cargo.toml`!

-   **No Manual Macros Needed:** You do **not** need to add `#[monitor_fn]` to your tasks to see their lifecycle. This tracing is completely automatic. You would only add `#[monitor_fn]` if you wanted to measure the *total execution time* of the task as a single block, in addition to seeing its detailed lifecycle.
