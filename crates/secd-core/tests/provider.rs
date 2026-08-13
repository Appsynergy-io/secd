#![allow(non_snake_case)]

use secd_core::{infer, providers};

const LOCKED_NAMES: &[&str] = &[
    "cloudflare",
    "aws",
    "s3",
    "github",
    "gitea",
    "gitlab",
    "slack",
    "digitalocean",
    "npm",
    "xai",
    "sendgrid",
    "pypi",
    "anthropic",
    "openai",
    "vault",
];

#[test]
fn T_PROVIDER_GITEA_ENV() {
    let gitea = providers()
        .iter()
        .find(|p| p.name == "gitea")
        .expect("gitea provider");
    let field = |key: &str| {
        gitea
            .fields
            .iter()
            .find(|f| f.key == key)
            .unwrap_or_else(|| panic!("gitea missing field {key}"))
    };
    assert_eq!(field("token").env, "GITEA_TOKEN");
    assert_eq!(field("url").env, "GITEA_URL");
    assert_eq!(field("user").env, "GITEA_USER");
}

#[test]
fn T_PROVIDER_VAULT_SHARES() {
    let vault = providers()
        .iter()
        .find(|p| p.name == "vault")
        .expect("vault provider");
    for i in 1..=5 {
        let key = format!("share_{i}");
        let field = vault
            .fields
            .iter()
            .find(|f| f.key == key)
            .unwrap_or_else(|| panic!("vault missing {key}"));
        assert!(field.secret, "{key} must be secret");
        assert!(field.optional, "{key} must be optional");
        assert_eq!(field.env, format!("VAULT_UNSEAL_SHARE_{i}"));
    }
}

#[test]
fn T_PROVIDER_COUNT() {
    let names: Vec<&str> = providers().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names.len(), 15);
    assert_eq!(names, LOCKED_NAMES);
}

#[test]
fn T_INFER_VAULT() {
    assert_eq!(infer(&["addr"]), Some("vault"));
    assert_eq!(infer(&["role_id", "secret_id"]), Some("vault"));
    assert_eq!(infer(&["share_1"]), Some("vault"));
}

#[test]
fn T_INFER_AMBIGUOUS() {
    assert_eq!(infer(&["token"]), None);
}
