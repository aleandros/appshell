#![cfg(not(feature = "dynamodb"))]
#![allow(clippy::unwrap_used)] // Test fixtures fail fast, including helper functions.
//! Executable dependency rules. These inspect Rust syntax, including macro tokens;
//! comments and string literals cannot produce matches or hide qualified imports.
use proc_macro2::{TokenStream, TokenTree};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use syn::visit::{self, Visit};

fn sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn identifiers(tokens: TokenStream, names: &mut BTreeSet<String>) {
    for token in tokens {
        match token {
            TokenTree::Ident(ident) => {
                names.insert(ident.to_string());
            }
            TokenTree::Group(group) => identifiers(group.stream(), names),
            _ => {}
        }
    }
}

struct Boundary<'a> {
    path: &'a str,
    errors: Vec<String>,
}
impl<'ast> Visit<'ast> for Boundary<'_> {
    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        if self.path.starts_with("infrastructure/repositories/") {
            let sql = node.value().to_uppercase();
            if sql
                .split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .any(|w| w == ["DELETE", "FROM"])
                || sql.split_whitespace().any(|w| w == "TRUNCATE")
            {
                self.errors
                    .push("hard deletion SQL is forbidden; use soft deletion".into());
            }
        }
        visit::visit_lit_str(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if let syn::Member::Named(field) = &node.member
            && ((self.path.starts_with("http/")
                && ["pool", "http", "dummy_password_hash"].contains(&field.to_string().as_str()))
                || (self.path.starts_with("contexts/") && field == "http"))
        {
            self.errors
                .push(format!("direct infrastructure field: {field}"));
        }
        visit::visit_expr_field(self, node);
    }
    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if node.path().is_ident("path") {
            self.errors
                .push("module path overrides bypass source layout".into());
        }
        visit::visit_attribute(self, node);
    }
}

fn violations(path: &str, source: &str, repository_methods: &BTreeSet<String>) -> Vec<String> {
    let syntax = syn::parse_file(source).expect("valid Rust source");
    let mut names = BTreeSet::new();
    identifiers(source.parse().unwrap(), &mut names);
    let mut boundary = Boundary {
        path,
        errors: Vec::new(),
    };
    boundary.visit_file(&syntax);
    if path.starts_with("contexts/") {
        let context = path.split('/').nth(1).unwrap();
        for method in names.intersection(repository_methods) {
            if !method.starts_with(&format!("{context}_")) {
                boundary.errors.push(format!(
                    "{context} must call the owning context service for {method}"
                ));
            }
        }
    }

    if ![
        "infrastructure/crypto.rs",
        "infrastructure/repositories/dynamodb/mod.rs",
    ]
    .contains(&path)
        && names.contains("new_v4")
    {
        boundary.errors.push(
            "generate database IDs in PostgreSQL; randomness belongs in crypto token generation"
                .into(),
        );
    }
    let domain = path.starts_with("domain/");
    let infrastructure = path.starts_with("infrastructure/");
    let mut denied = vec!["include", "include_str", "include_bytes"];
    if !infrastructure {
        denied.extend([
            "lettre",
            "diesel",
            "diesel_migrations",
            "sql_query",
            "PgConnection",
            "RunQueryDsl",
            "QueryableByName",
        ]);
    }
    if path.starts_with("http/") {
        denied.extend([
            "infrastructure",
            "repositories",
            "db",
            "crypto",
            "appshell_domain",
        ]);
    }
    if path.starts_with("contexts/") {
        denied.extend([
            "axum",
            "reqwest",
            "tower_http",
            "sql_types",
            "db",
            "new_v4",
            "Utc",
            "fs",
            "net",
            "env",
            "process",
            "thread",
            "SystemTime",
            "Instant",
        ]);
    }
    if domain {
        denied.extend([
            "appshell_api",
            "tokio",
            "axum",
            "reqwest",
            "serde",
            "utoipa",
            "tracing",
            "fs",
            "net",
            "io",
            "env",
            "process",
            "thread",
            "SystemTime",
            "Instant",
            "Utc",
            "rand",
            "rand_core",
        ]);
    }
    if infrastructure
        && !path.starts_with("infrastructure/repositories/")
        && path != "infrastructure/db.rs"
    {
        denied.extend(["sql_query", "PgConnection", "RunQueryDsl"]);
    }
    for name in denied {
        if names.contains(name) {
            boundary
                .errors
                .push(format!("{name} is forbidden in this layer"));
        }
    }
    if !(domain
        || infrastructure
        || path.starts_with("contexts/")
        || path.starts_with("http/")
        || [
            "lib.rs",
            "main.rs",
            "config.rs",
            "jobs.rs",
            "error.rs",
            "models.rs",
            "bin/export-openapi.rs",
            "bin/bootstrap-admin.rs",
            "bin/lambda.rs",
        ]
        .contains(&path))
    {
        boundary
            .errors
            .push("unclassified source file; choose a layer and update its rules".into());
    }
    boundary.errors
}

#[test]
fn production_dependencies_respect_layers_and_contexts() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let domain = api.join("../../domain/src");
    let mut repository_methods = BTreeSet::new();
    for path in sources(&api.join("infrastructure/repositories")) {
        let syntax = syn::parse_file(&fs::read_to_string(path).unwrap()).unwrap();
        for item in syntax.items {
            if let syn::Item::Impl(item) = item {
                for item in item.items {
                    if let syn::ImplItem::Fn(method) = item
                        && !["transaction", "actor", "generated_id"]
                            .contains(&method.sig.ident.to_string().as_str())
                    {
                        repository_methods.insert(method.sig.ident.to_string());
                    }
                }
            }
        }
    }
    let mut failures = Vec::new();
    for (root, prefix) in [(&api, ""), (&domain, "domain/")] {
        for path in sources(root) {
            let relative = format!("{prefix}{}", path.strip_prefix(root).unwrap().display());
            for error in violations(
                &relative,
                &fs::read_to_string(&path).unwrap(),
                &repository_methods,
            ) {
                failures.push(format!("{relative}: {error}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn rules_reject_bypasses_and_allow_literals() {
    let methods = BTreeSet::from(["billing_subscription".into()]);
    assert!(
        !violations(
            "contexts/identity/service.rs",
            "fn f() { UnitOfWork::billing_subscription(c, org); }",
            &methods
        )
        .is_empty()
    );

    for (path, source) in [
        (
            "infrastructure/repositories/invalid.rs",
            "fn f(){ sql_query(\"DELETE FROM users\"); }",
        ),
        (
            "infrastructure/repositories/invalid.rs",
            "fn f(){ sql_query(\"TRUNCATE users\"); }",
        ),
        (
            "infrastructure/repositories/invalid.rs",
            "fn f(){ uuid::Uuid::new_v4(); }",
        ),
        ("http/invalid.rs", "use diesel as storage;"),
        ("contexts/identity/service.rs", "use lettre::Message;"),
        (
            "http/invalid.rs",
            "fn f() { let pool = state.pool.clone(); }",
        ),
        (
            "http/invalid.rs",
            "fn f() { wrapper!(diesel::sql_query(\"SELECT 1\")); }",
        ),
        (
            "contexts/identity/service.rs",
            "fn f() { c.billing_subscription(org); }",
        ),
        (
            "contexts/billing/service.rs",
            "fn f() { state.http.get(url); }",
        ),
        ("domain/rule.rs", "use std::{fs as storage};"),
        ("domain/rule.rs", "fn f() { std::time::SystemTime::now(); }"),
        (
            "http/invalid.rs",
            "#[path = \"../infrastructure/db.rs\"] mod other;",
        ),
        ("http/invalid.rs", "include!(\"other.rs\");"),
        ("helpers.rs", "fn f() {}"),
    ] {
        assert!(
            !violations(path, source, &methods).is_empty(),
            "accepted {path}: {source}"
        );
    }
    assert!(
        violations(
            "domain/rule.rs",
            "// diesel::sql_query\nfn message() -> &'static str { \"std::fs\" }",
            &methods
        )
        .is_empty()
    );
    assert!(
        violations(
            "http/valid.rs",
            "use crate::contexts::identity as service;",
            &methods
        )
        .is_empty()
    );
}
