#[macro_export]
// This macro is useful for tracing the execution flow and timing of code sections. Use only appropriately with rustmeter-cli
///
/// The return value of the code block is passed through, allowing the macro to be
/// used in assignments (see Example 2).
///
/// # Arguments
///
/// * `$name`: A string literal describing the scope name (interned by `defmt`).
/// * `$body`: The code block enclosed in curly braces `{ ... }`.
///
/// # Warning
///
/// If the code block is exited early via `return`, `break`, or `continue`,
/// the concluding `STOP` message will **not** be sent.
///
/// # Examples
///
/// ```rust
///// Example 1: Simple block without a return value (Type `()`)
///monitor_scoped!("SensorInit", {
///    // Your code goes here
///    let i = 0;
///    do_something(i);
///});
///
///// Example 2: Block with a return value
///// The value of the last expression (a + b) is stored in 'result'.
///let result = monitor_scoped!("Calculation", {
///    let a = 10;
///    let b = 20;
///    a + b
///});
/// ```
macro_rules! monitor_scoped {
    ($name:literal, $body:block) => {{
        let core_id = rustmeter_beacon::get_current_core_id();
        defmt::info!(
            "@EVENT_MONITOR_START(function_name={=istr},core_id={})",
            defmt::intern!($name),
            core_id
        );

        let result = { $body };
        defmt::info!(
            "@EVENT_MONITOR_END(function_name={=istr},core_id={})",
            defmt::intern!($name),
            core_id
        );

        result
    }};
}
