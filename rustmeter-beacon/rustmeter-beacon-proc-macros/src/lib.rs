use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemFn, LitStr, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

extern crate proc_macro;

/// Helper struct to parse arguments for the `monitor_fn` attribute macro
struct MonitorArgs {
    name: Option<String>,
}

impl Parse for MonitorArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = None;
        if input.is_empty() {
            return Ok(MonitorArgs { name });
        }

        // Case 1: #[monitor_fn("Name")]
        // `lookahead` checks if the next token is a string literal
        if input.peek(syn::LitStr) {
            let lit: LitStr = input.parse()?;
            name = Some(lit.value());
        }
        // Case 2: Key-Value Pair: #[monitor_fn(name = "Name")]
        else if input.peek(syn::Ident) {
            let key: Ident = input.parse()?;
            if key == "name" {
                input.parse::<Token![=]>()?; // Consume the '='
                let lit: LitStr = input.parse()?;
                name = Some(lit.value());
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    "Unknown argument (expected 'name')",
                ));
            }
        }

        // More arguments could be parsed here in the future

        Ok(MonitorArgs { name })
    }
}

/// Instruments a function to log execution for rustmeter
///
/// This attribute macro wraps the decorated function to log specific `@EVENT_MONITOR`
/// messages before execution starts and after it finishes. It captures the function name
/// (or a custom name) and the current core ID.
///
/// It supports both synchronous and `async` functions.
///
/// # Arguments
///
/// The macro accepts an optional name argument to override the default function name in the logs.
///
/// * `#[monitor_fn]` - Uses the name of the function.
/// * `#[monitor_fn("custom_name")]` - Uses the provided string literal.
/// * `#[monitor_fn(name = "custom_name")]` - Explicit key-value syntax.
///
/// # Examples
///
/// Basic usage using the function's name:
///
/// ```rust
/// #[monitor_fn]
/// fn process_data(data: u8) {
///     // Function implementation
/// }
/// ```
#[proc_macro_attribute]
pub fn monitor_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let args = parse_macro_input!(attr as MonitorArgs);

    let fn_name = &input.sig.ident;
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;
    let attrs = &input.attrs; // Important: Keep other attributes (e.g., #[inline])

    let mut output_name = fn_name.to_string();

    // Handle output name from args (if provided)
    if let Some(custom_name) = args.name {
        output_name = custom_name;
    }

    if input.sig.asyncness.is_some() {
        // ASYNC FUNCTION
        quote! {
        let core_id = rustmeter_beacon::core_id::get_current_core_id();
        async move {
                // defmt::info!("@EVENT_MONITOR_START(function_name={=istr},core_id={})", defmt::intern!(#output_name), core_id);
                let result = { #block };
                // defmt::info!("@EVENT_MONITOR_END(function_name={=istr},core_id={})", defmt::intern!(#output_name), core_id);
                result
            }
        }
        .into()
    } else {
        // SYNC FUNCTION
        quote! {
            #(#attrs)*
            #vis #sig {
                {
                    let core_id = rustmeter_beacon::core_id::get_current_core_id();

                    // Get or register monitor ID
                    use rustmeter_beacon::monitors::VALUE_MONITOR_REGISTRY;
                    let (local_id, registered_newly) = rustmeter_beacon::get_static_id_by_registry!(
                        rustmeter_beacon::monitors::CODE_MONITOR_REGISTRY
                    );

                    // Send TypeDefinition event if newly registered
                    if registered_newly {
                        let fn_addr = #fn_name as usize;
                        let payload = rustmeter_beacon::protocol::TypeDefinitionPayload::FunctionMonitor {
                            monitor_id: local_id as u8,
                            fn_address: fn_addr as u32,
                        };
                        rustmeter_beacon::tracing::write_tracing_event(
                            rustmeter_beacon::protocol::EventPayload::TypeDefinition(payload)
                        );
                    
                        rustmeter_beacon::monitors::defmt_trace_new_function_monitor(#output_name, local_id);
                    }

                    // Create guard to signal end of scope
                    let _guard = rustmeter_beacon::monitors::DropGuard::new(|| {
                        // Create and send MonitorEnd event
                        let payload = match core_id {
                            0 => rustmeter_beacon::protocol::EventPayload::MonitorEndCore0 {},
                            1 => rustmeter_beacon::protocol::EventPayload::MonitorEndCore1 {},
                            _ => rustmeter_beacon::core_id::unreachable_core_id(core_id),
                        };
                        rustmeter_beacon::tracing::write_tracing_event(payload);
                    });

                    // Send MonitorStart event (after guard-created to lower tracing impact on measured scope)
                    let payload = match core_id {
                        0 => rustmeter_beacon::protocol::EventPayload::MonitorStartCore0 {
                            monitor_id: local_id as u8
                        },
                        1 => rustmeter_beacon::protocol::EventPayload::MonitorStartCore1 {
                            monitor_id: local_id as u8
                        },
                        _ => rustmeter_beacon::core_id::unreachable_core_id(core_id),
                    };
                    rustmeter_beacon::tracing::write_tracing_event(payload);
                

                    // Execute original function body
                    { #block }
                }
            }           
        }.into()
    }
}
