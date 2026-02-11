use std::hash::{DefaultHasher, Hash, Hasher};

use proc_macro2::Span;
use quote::ToTokens;
use syn::{
    ItemFn, LitStr, Result, Token,
    parse::{Parse, ParseStream},
};

pub mod transformer;

pub struct ScopedMonitorInput {
    pub name: LitStr,
    _comma: Token![,],
    pub block: syn::Block,
}

impl Parse for ScopedMonitorInput {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(ScopedMonitorInput {
            name: input.parse()?,
            _comma: input.parse()?,
            block: input.parse()?,
        })
    }
}

pub fn contains_await(stmt: &syn::Stmt) -> bool {
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

pub fn is_step_stmt(stmt: &syn::Stmt) -> bool {
    get_step_label(stmt).is_some()
}

pub fn get_step_label(stmt: &syn::Stmt) -> Option<String> {
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

pub fn scoped_disambiguator(input: &ScopedMonitorInput) -> u64 {
    let mut hasher = DefaultHasher::new();

    // use scope name + body
    input.name.value().hash(&mut hasher);
    input.block.to_token_stream().to_string().hash(&mut hasher);

    // use filename, line and column
    let source = Span::call_site().unwrap().source();
    source.file().hash(&mut hasher);
    source.line().hash(&mut hasher);
    source.column().hash(&mut hasher);

    hasher.finish()
}

pub fn fn_disambiguator(input: &ItemFn) -> u64 {
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
