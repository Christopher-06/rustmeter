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

    // TODO: Send one event when function is done and measure duration from start ==> less defmt messages BUT long computation inside function do not get logged when
    //       function is running for a long time and we exit while it is still running
    //          - which timestamp method to use for that?

    if input.sig.asyncness.is_some() {
        // ASYNC FUNCTION
        quote! {
            let core_id = rustmeter_beacon::get_current_core_id();
            async move {
                    defmt::info!("@EVENT_MONITOR_START(function_name={=istr},core_id={})", defmt::intern!(#output_name), core_id);
                    let result = { #block };
                    defmt::info!("@EVENT_MONITOR_END(function_name={=istr},core_id={})", defmt::intern!(#output_name), core_id);
                    result
                }
            }.into()
    } else {
        // SYNC FUNCTION
        quote! {
            #(#attrs)*
            #vis #sig {
                let core_id = rustmeter_beacon::get_current_core_id();
                defmt::info!("@EVENT_MONITOR_START(function_name={=istr},core_id={})", defmt::intern!(#output_name), core_id);
                let result = (move || { #block })();
                defmt::info!("@EVENT_MONITOR_END(function_name={=istr},core_id={})", defmt::intern!(#output_name), core_id);
                result
            }
        }.into()
    }
}
