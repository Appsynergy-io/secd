//! `secd run --with P=B` maps a bundle whose fields use the upstream's own spelling. Vault's
//! AppRole API names them `role-id`/`secret-id`, so a bundle grouped from `local/vault/role-id`
//! and `local/vault/secret-id` must still reach a child as `VAULT_ROLE_ID`/`VAULT_SECRET_ID`.
use std::collections::BTreeMap;

use secd::policy::{self, Bundle};

#[test]
fn hyphenated_vault_fields_map_to_env() {
    let mut fields = BTreeMap::new();
    fields.insert("role-id".to_string(), "rid".to_string());
    fields.insert("secret-id".to_string(), "sid".to_string());
    let bundle = Bundle {
        name: "local/vault".into(),
        provider: "vault".into(),
        fields,
    };
    let specs = vec![policy::parse_with("vault=local/vault").expect("spec")];
    let env = policy::apply_with(&specs, std::slice::from_ref(&bundle)).expect("map");
    assert_eq!(env.get("VAULT_ROLE_ID").map(String::as_str), Some("rid"));
    assert_eq!(env.get("VAULT_SECRET_ID").map(String::as_str), Some("sid"));
}

#[test]
fn underscore_fields_still_map() {
    let mut fields = BTreeMap::new();
    fields.insert("role_id".to_string(), "rid".to_string());
    fields.insert("secret_id".to_string(), "sid".to_string());
    let bundle = Bundle {
        name: "local/vault".into(),
        provider: "vault".into(),
        fields,
    };
    let specs = vec![policy::parse_with("vault=local/vault").expect("spec")];
    let env = policy::apply_with(&specs, std::slice::from_ref(&bundle)).expect("map");
    assert_eq!(env.get("VAULT_ROLE_ID").map(String::as_str), Some("rid"));
    assert_eq!(env.get("VAULT_SECRET_ID").map(String::as_str), Some("sid"));
}

#[test]
fn individual_and_custom_fields_map_without_provider_inference() {
    use secd::policy::Entry;
    use secd::Secret;
    let entries = vec![
        Entry {
            name: "prod/password".into(),
            value: Secret::new(b"  exact\n".to_vec()),
            meta: serde_json::json!({}),
        },
        Entry {
            name: "prod/custom".into(),
            value: Secret::new(br#"{"client_id":"id","client_secret":" secret "}"#.to_vec()),
            meta: serde_json::json!({"kind":"bundle"}),
        },
    ];
    let specs = [
        "PASSWORD=prod/password",
        "CLIENT_SECRET=prod/custom#client_secret",
    ]
    .map(|s| policy::parse_with(s).unwrap());
    let env = policy::apply_entries(&specs, &entries).unwrap();
    assert_eq!(env["PASSWORD"], "  exact\n");
    assert_eq!(env["CLIENT_SECRET"], " secret ");
    let missing = [policy::parse_with("VALUE=prod/custom#missing").unwrap()];
    assert!(policy::apply_entries(&missing, &entries).is_err());
    let collision = ["github=prod/github", "GITHUB_TOKEN=prod/password"]
        .map(|s| policy::parse_with(s).unwrap());
    assert!(policy::check_with_collision(&collision).is_err());
}
