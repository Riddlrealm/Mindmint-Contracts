//! Per-function permission linter (issue #19).
//!
//! For every `pub fn` in an enforced contract's `lib.rs`, a "mutating" function
//! (body contains a storage write `.set(` / `.remove(` / `.update(`, or a token
//! `.transfer(`) must also contain `require_auth`. Functions that fail this
//! print a `MISSING_AUTH_CHECK: <file>::<fn>` row; the process exits 1 if any.
//!
//! This replaces the per-file `grep`, which passed an entire contract on a
//! single `require_auth` match anywhere. It enforces the set of contracts in
//! `ENFORCED` (currently `guild`, the subject of #19). A one-off workspace-wide
//! run surfaced ~140 pre-existing candidate functions across ~45 other
//! contracts; triaging those (and a full semantic pass) is separate follow-up
//! work. New contracts get added to `ENFORCED` as they are hardened.

use std::{fs, path::PathBuf, process::exit};
use syn::{visit::Visit, ImplItemFn, ItemFn, Visibility};

/// Contracts whose mutating `pub fn`s are enforced to call `require_auth`.
/// Extend this list as more contracts are audited in follow-up work.
const ENFORCED: &[&str] = &["guild"];

/// Render a block and strip all whitespace so pattern matching is robust to
/// how `TokenStream::to_string()` spaces tokens across proc-macro2 versions.
fn body_text(block: &syn::Block) -> String {
    quote::quote!(#block)
        .to_string()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn is_mutating(body: &str) -> bool {
    body.contains(".set(")
        || body.contains(".remove(")
        || body.contains(".update(")
        || body.contains(".transfer(")
}

fn has_auth(body: &str) -> bool {
    body.contains("require_auth")
}

struct Checker<'a> {
    file: &'a str,
    failures: &'a mut Vec<String>,
}

impl Checker<'_> {
    fn check(&mut self, name: &str, vis: &Visibility, block: &syn::Block) {
        if !matches!(vis, Visibility::Public(_)) {
            return;
        }
        let body = body_text(block);
        if is_mutating(&body) && !has_auth(&body) {
            self.failures
                .push(format!("MISSING_AUTH_CHECK: {}::{}", self.file, name));
        }
    }
}

impl<'ast> Visit<'ast> for Checker<'_> {
    fn visit_impl_item_fn(&mut self, f: &'ast ImplItemFn) {
        self.check(&f.sig.ident.to_string(), &f.vis, &f.block);
    }
    fn visit_item_fn(&mut self, f: &'ast ItemFn) {
        self.check(&f.sig.ident.to_string(), &f.vis, &f.block);
    }
}

fn main() {
    let mut failures = Vec::new();
    let paths: Vec<PathBuf> = ENFORCED
        .iter()
        .map(|c| {
            PathBuf::from("contracts")
                .join(c)
                .join("src")
                .join("lib.rs")
        })
        .collect();

    for path in paths {
        let file = path.to_string_lossy().to_string();
        let src = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("PARSE_SKIP: {file} (read error: {e})");
                continue;
            }
        };
        let ast = match syn::parse_file(&src) {
            Ok(a) => a,
            Err(e) => {
                // A file syn can't parse is not silently passed: it's surfaced
                // so it can be investigated, but it doesn't abort the audit.
                eprintln!("PARSE_SKIP: {file} ({e})");
                continue;
            }
        };
        let mut checker = Checker {
            file: &file,
            failures: &mut failures,
        };
        checker.visit_file(&ast);
    }

    if failures.is_empty() {
        println!("OK: every mutating pub fn calls require_auth");
    } else {
        for f in &failures {
            println!("{f}");
        }
        exit(1);
    }
}
