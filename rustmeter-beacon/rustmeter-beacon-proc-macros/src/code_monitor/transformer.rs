use std::collections::HashMap;

use quote::ToTokens;
use syn::{
    Expr, ExprAwait, Stmt, parse_quote,
    visit_mut::{self, VisitMut},
};

use crate::code_monitor::{contains_await, get_step_label, is_step_stmt};

/// This inserts a starting step! and after each await block. Can create duplicates!
pub struct StepInsertTransformer {
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
pub struct StepDedupTransformer {
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
pub struct MonitorTransformer {
    fn_name: String,
    prev_monitor_guard: Option<String>,
    prev_async_stmt: Option<(u16, Stmt)>, // prev State, Async Statement
    next_state_index: u16,
    pub state_names: HashMap<u16, String>,
    pub state_transitions: HashMap<u16, HashMap<u16, String>>, // (from, to) -> transition label (Stmt: XXX.wait().await then "xxx.wait()")
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
    fn visit_stmt_mut(&mut self, i: &mut syn::Stmt) {
        // Ignore monitor_scoped!
        if let syn::Stmt::Macro(m) = i {
            if m.mac.path.is_ident("monitor_scoped") {
                return;
            }
        }

        // Standard-Verarbeitung für alle anderen Statements
        visit_mut::visit_stmt_mut(self, i);
    }

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
