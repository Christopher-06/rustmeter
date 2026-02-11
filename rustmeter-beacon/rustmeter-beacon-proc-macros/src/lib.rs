#![feature(proc_macro_span)]
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use rustmeter_beacon_core::code_monitor::FunctionMetadata;
use syn::{
    Error, Expr, ExprAwait, Ident, ItemFn, LitStr, Result, Stmt, Token,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    visit_mut::{self, VisitMut, visit_stmt_mut},
};

extern crate proc_macro;

/// This inserts a starting step! and after each await block. Can create duplicates!
struct StepInsertTransformer {
    fn_name: String,
    segment_index: usize,
}

impl StepInsertTransformer {
    pub fn new(fn_name: String) -> Self {
        Self {
            fn_name,
            segment_index: 0,
        }
    }

    /// Step naming: "fn_name:stepX"
    fn auto_step_label(&mut self) -> String {
        let label = format!("{}:step{}", self.fn_name, self.segment_index);
        self.segment_index += 1;
        label
    }

    fn create_step_stmt(&self, label: String) -> Stmt {
        // parse_quote!(rustmeter_beacon::step!(#label);)
        parse_quote!(step!(#label);)
    }
}

impl VisitMut for StepInsertTransformer {
    fn visit_block_mut(&mut self, i: &mut syn::Block) {
        let mut new_stmts = Vec::new();

        // First step at the beginning of the function (only for the outermost block)
        if self.segment_index == 0 {
            let start_name = self.auto_step_label();
            new_stmts.push(self.create_step_stmt(start_name));
        }

        // Insert step!() after each statement that contains an await
        let mut stmts_iter = std::mem::take(&mut i.stmts).into_iter().peekable();
        while let Some(mut stmt) = stmts_iter.next() {
            self.visit_stmt_mut(&mut stmt); // go deeper to find nested awaits

            // Check for async and reinsert
            let contains_await = contains_await(&stmt);
            new_stmts.push(stmt);

            // Add step!()
            if contains_await {
                let label_name = self.auto_step_label();
                new_stmts.push(self.create_step_stmt(label_name));
            }
        }

        i.stmts = new_stmts;
    }
}

/// Deduplicate step!() calls by only keeping the last one of contiguous step!() calls.
/// This needs to be done because StepInsertTransformer can create multiple step!() calls for nested awaits and when user also uses step!() manually. This transformer should be run after StepInsertTransformer to clean up the duplicates.
struct StepDedupTransformer {
    prev_step: Option<Stmt>,
}

impl StepDedupTransformer {
    pub fn new() -> Self {
        Self { prev_step: None }
    }
}

impl VisitMut for StepDedupTransformer {
    fn visit_block_mut(&mut self, i: &mut syn::Block) {
        let mut new_stmts = Vec::new();

        let mut stmts_iter = std::mem::take(&mut i.stmts).into_iter().peekable();
        while let Some(mut stmt) = stmts_iter.next() {
            self.visit_stmt_mut(&mut stmt); // go deeper to find nested

            let current_is_step = is_step_stmt(&stmt);
            if current_is_step {
                // Is step (first or overwrite prev)
                self.prev_step = Some(stmt.clone());
            } else {
                // No step, check if any step was back there
                if let Some(step_stmt) = self.prev_step.take() {
                    new_stmts.push(step_stmt);
                }

                new_stmts.push(stmt);
            }
        }

        i.stmts = new_stmts;
    }
}

#[derive(Debug)]
/// insert monitoring instruments. Start new and drop prev on step!() and drop on await expr
struct MonitorTransformer {
    fn_name: String,
    prev_monitor_guard: Option<String>,
    prev_async_stmt: Option<(u16, Stmt)>, // prev State, Async Statement
    next_state_index: u16,
    state_names: HashMap<u16, String>,
    state_transitions: HashMap<u16, HashMap<u16, String>>, // (from, to) -> transition label (Stmt: XXX.wait().await then "xxx.wait()")
}

impl MonitorTransformer {
    pub fn new(fn_name: String) -> Self {
        MonitorTransformer {
            fn_name,
            prev_monitor_guard: None,
            next_state_index: 0,
            prev_async_stmt: None,
            state_names: HashMap::new(),
            state_transitions: HashMap::new(),
        }
    }

    /// Drop prev monitor guard if any. This is needed for async points
    fn drop_prev_guard(&mut self, new_stmts: &mut Vec<Stmt>) {
        if let Some(_) = self.prev_monitor_guard.take() {
            new_stmts.push(parse_quote!(
                __CODE_MONITOR_GUARD.end();
            ));
        }
    }

    /// Create new monitor guard and place it into prev monitor guard.
    /// The message will automatically overwrite the previously state
    fn create_new_monitor_guard(&mut self, mut label: String, new_stmts: &mut Vec<Stmt>) {
        // Enfore fn name prefix for better readability in the monitor
        if !label.starts_with(&format!("{}:", self.fn_name)) {
            label = format!("{}:{}", self.fn_name, label);
        }

        // Enter new monitor guard
        let state_index = self.next_state_index;
        new_stmts.push(parse_quote!(
            __CODE_MONITOR_GUARD.activate(__RUSTMETER_FN_MONITOR_ID, #state_index);
        ));

        // Keep log
        self.prev_monitor_guard = Some(label.clone());
        self.state_names.insert(self.next_state_index, label);
        self.next_state_index += 1;
    }

    fn add_transition(&mut self, from_state: u16, to_state: u16, label: String) {
        self.state_transitions
            .entry(from_state)
            .or_insert_with(HashMap::new)
            .insert(to_state, label);
    }

    fn async_statement_to_transition_label(stmt: &Stmt) -> Option<String> {
        if let Stmt::Expr(e, _) = stmt {
            if let Expr::Await(ExprAwait { base, .. }) = e {
                return Some(base.to_token_stream().to_string().replace(" ", ""));
            }
        }

        None
    }
}

impl VisitMut for MonitorTransformer {
    fn visit_block_mut(&mut self, i: &mut syn::Block) {
        let mut new_stmts = Vec::new();

        let mut stmts_iter = std::mem::take(&mut i.stmts).into_iter().peekable();
        while let Some(mut stmt) = stmts_iter.next() {
            self.visit_stmt_mut(&mut stmt); // go deeper to find nested

            // Drop prev monitor on awaits
            let contains_await: bool = contains_await(&stmt);
            if contains_await {
                self.prev_async_stmt = Some((self.next_state_index - 1, stmt.clone()));
                self.drop_prev_guard(&mut new_stmts);
            }

            // Start Monitor on Step or call orig statement
            let step_name = get_step_label(&stmt);
            if let Some(label) = step_name {
                // Enter transition if we come from async statement
                if let Some((from_state, async_stmt)) = self.prev_async_stmt.take() {
                    let trans_label = Self::async_statement_to_transition_label(&async_stmt)
                        .unwrap_or_else(|| {
                            format!("StateTransition{}to{}", from_state, self.next_state_index)
                        });

                    self.add_transition(from_state, self.next_state_index, trans_label);
                }

                self.create_new_monitor_guard(label, &mut new_stmts);
            } else {
                new_stmts.push(stmt);
            }
        }

        i.stmts = new_stmts;
    }
}

fn contains_await(stmt: &syn::Stmt) -> bool {
    struct AwaitChecker(bool);
    impl<'ast> syn::visit::Visit<'ast> for AwaitChecker {
        fn visit_expr_await(&mut self, _: &'ast syn::ExprAwait) {
            self.0 = true;
        }
    }
    let mut checker = AwaitChecker(false);
    syn::visit::visit_stmt(&mut checker, stmt);
    checker.0
}

fn is_step_stmt(stmt: &syn::Stmt) -> bool {
    get_step_label(stmt).is_some()
}

fn get_step_label(stmt: &syn::Stmt) -> Option<String> {
    if let &syn::Stmt::Macro(ref m) = stmt {
        // TODO: Handle rustmeter_beacon::step! as well
        if m.mac.path.is_ident("step") {
            if let Ok(label) = m.mac.parse_body::<LitStr>() {
                return Some(label.value());
            }

            // TODO: What when no String? Empty or other type!
        }
    }

    None
}

fn fn_disambiguator(input: &ItemFn) -> u64 {
    let mut hasher = DefaultHasher::new();

    // use fn name + body
    input.sig.ident.to_string().hash(&mut hasher);
    input.block.to_token_stream().to_string().hash(&mut hasher);

    // use filename, line and column
    let source = Span::call_site().unwrap().source();
    source.file().hash(&mut hasher);
    source.line().hash(&mut hasher);
    source.column().hash(&mut hasher);

    hasher.finish()
}

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

#[unsafe(no_mangle)]
fn write_tracing_data(_data: &[u8]) {}
