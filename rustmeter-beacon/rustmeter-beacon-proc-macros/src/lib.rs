#![feature(proc_macro_span)]

mod code_monitor;

use proc_macro::TokenStream;
use quote::quote;
use rustmeter_beacon_core::code_monitor::FunctionMetadata;
use syn::{Error, ItemFn, parse_macro_input, parse_quote, visit_mut::VisitMut};

use crate::code_monitor::{
    ScopedMonitorInput, contains_await, fn_disambiguator, scoped_disambiguator,
    transformer::{MonitorTransformer, StepDedupTransformer, StepInsertTransformer},
};

extern crate proc_macro;

/// Instruments a function to log execution for rustmeter. Can be used on sync or async functions.
/// Use the step! macro to name individual steps inside. In async functions after each await a new step is
/// automatically inserted. If you want to name it acordingly, you should use the step! macro right after the await.
///
///  # Example
///
/// ```rust
/// #[monitor_fn]
/// fn process_data(data: u8) {
///     // Function implementation
/// }
/// ```
#[proc_macro_attribute]
pub fn monitor_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as ItemFn);
    let fn_name = input.sig.ident.to_string();

    // Automatically insert step! after awaits (can create duplicates)
    let mut transformer = StepInsertTransformer::new(fn_name.clone());
    transformer.visit_block_mut(&mut input.block);

    // Dedup multiple step statements
    let mut transformer = StepDedupTransformer::new();
    transformer.visit_block_mut(&mut input.block);

    // Add monitoring instruments
    let mut transformer = MonitorTransformer::new(fn_name.clone());
    transformer.visit_block_mut(&mut input.block);

    // Prepare metadata
    let metadata = FunctionMetadata {
        fn_name,
        disambiguator: fn_disambiguator(&input),
        is_async: input.sig.asyncness.is_some(),
        state_names: transformer.state_names,
        state_transition_names: transformer.state_transitions,
    };
    let metdata_link_section = format!(".rustmeter_fn_metadata.{}", metadata.disambiguator);
    let metadata_export_name =
        serde_json::to_string(&metadata).expect("Failed to serialize function metadata");

    // Inject CodeMonitorGuard and metadata at beginning
    input.block.stmts.insert(
        0,
        parse_quote!(
            let (__RUSTMETER_FN_MONITOR_ID, mut __CODE_MONITOR_GUARD) = {
                let code_guard = rustmeter_beacon::code_monitor::CodeMonitorGuard::new();

                // Define metadata
                #[unsafe(link_section = #metdata_link_section)]
                #[unsafe(export_name = #metadata_export_name)]
                static RUSTMETER_FN_METADATA: u8 = 0;

                // use mem address of metadata as monitor id (later it will be used with VARINT)
                let fn_monitor_id = unsafe { &RUSTMETER_FN_METADATA as *const u8 as u16 };

                (fn_monitor_id, code_guard)
            };
        ),
    );

    quote! {
        #input
    }
    .into()
}

/// Macro to monitor a code scope with Rustmeter Beacon (only synchronous!). Can also be
/// used with step! macro!
///
/// ## Parameters
/// - $name: A string literal representing the name of the scope to be monitored.
/// - $body: A block of code representing the scope to be monitored. Must be synchronous.
///
/// # Examples
/// ```rust,no_run
/// fn matrix_multiply(a: &Matrix, b: &Matrix) -> Matrix {
///     // prepare or anything
///
///     let result = monitor_scoped!("matrix_mul", {
///         a * b
///     });
///
///     // finalize or anything
///     result
/// }
#[proc_macro]
pub fn monitor_scoped(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ScopedMonitorInput);
    let disambiguator = scoped_disambiguator(&input);
    let mut block = input.block;
    let name_val = input.name.value();

    // Check for syncness
    for stmt in block.stmts.iter() {
        if contains_await(stmt) {
            return Error::new_spanned(
                stmt,
                "monitor_scoped! does not support async code. Please use monitor_fn for async functions or scopes with awaits.",
            )
            .to_compile_error()
            .into();
        }
    }

    // Automatically insert step! after awaits (can create duplicates)
    let mut transformer = StepInsertTransformer::new(name_val.clone());
    transformer.visit_block_mut(&mut block);

    // Dedup multiple step statements
    let mut transformer = StepDedupTransformer::new();
    transformer.visit_block_mut(&mut block);

    // Add monitoring instruments
    let mut transformer = MonitorTransformer::new(name_val.clone());
    transformer.visit_block_mut(&mut block);

    // Prepare metadata
    let metadata = FunctionMetadata {
        fn_name: name_val.clone(),
        disambiguator,
        is_async: false,
        state_names: transformer.state_names,
        state_transition_names: transformer.state_transitions,
    };
    let metdata_link_section = format!(".rustmeter_fn_metadata.{}", metadata.disambiguator);
    let metadata_export_name =
        serde_json::to_string(&metadata).expect("Failed to serialize function metadata");

    // Inject CodeMonitorGuard and metadata at beginning
    block.stmts.insert(
        0,
        parse_quote!(
            let (__RUSTMETER_FN_MONITOR_ID, mut __CODE_MONITOR_GUARD) = {
                let code_guard = rustmeter_beacon::code_monitor::CodeMonitorGuard::new();

                // Define metadata
                #[unsafe(link_section = #metdata_link_section)]
                #[unsafe(export_name = #metadata_export_name)]
                static RUSTMETER_FN_METADATA: u8 = 0;

                // use mem address of metadata as monitor id (later it will be used with VARINT)
                let fn_monitor_id = unsafe { &RUSTMETER_FN_METADATA as *const u8 as u16 };

                (fn_monitor_id, code_guard)
            };
        ),
    );

    quote! {
        #block
    }
    .into()
}

#[unsafe(no_mangle)]
fn write_tracing_data(_data: &[u8]) {}
