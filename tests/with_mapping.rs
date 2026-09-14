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
