use std::sync::OnceLock;

/// Built-in provider schema. `name` is the locked identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provider {
    pub name: String,
    pub title: String,
    pub fields: Vec<Field>,
}

/// Custom account schema. Persisted as `{name, title, fields}`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomProvider {
    pub name: String,
    pub title: String,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    pub key: String,
    pub secret: bool,
    pub optional: bool,
    pub env: String,
}

/// The 15 locked built-in providers.
pub fn providers() -> &'static [Provider] {
    static CELL: OnceLock<Vec<Provider>> = OnceLock::new();
    CELL.get_or_init(builtins).as_slice()
}

/// Infer a provider from field keys. Distinctive keys win; shared keys (e.g. only `token`) yield nothing.
pub fn infer(keys: &[&str]) -> Option<&'static str> {
    if keys.is_empty() {
        return None;
    }
    let hits: Vec<&Provider> = providers()
        .iter()
        .filter(|p| keys.iter().all(|k| p.fields.iter().any(|f| f.key == *k)))
        .collect();
    match hits.as_slice() {
        [only] => Some(only.name.as_str()),
        _ => None,
    }
}

/// Schema-ordered pairs of trimmed, non-empty values. `None` when a required
/// field is empty. The rule the web console spells as `buildPayload`.
///
/// Order is load-bearing: the console seals its payload in schema order and
/// records the same order in the entry meta, so a sorted map would diverge.
pub fn build_payload(
    fields: &[Field],
    values: &[(String, String)],
) -> Option<Vec<(String, String)>> {
    let mut out = Vec::with_capacity(fields.len());
    for f in fields {
        let raw = values
            .iter()
            .find(|(k, _)| *k == f.key)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default();
        let v = raw.trim();
        if v.is_empty() {
            if f.optional {
                continue;
            }
            return None;
        }
        out.push((f.key.clone(), v.to_string()));
    }
    Some(out)
}

fn builtins() -> Vec<Provider> {
    vec![
        Provider {
            name: "cloudflare".to_string(),
            title: "Cloudflare".to_string(),
            fields: vec![
                req("account_id", "CLOUDFLARE_ACCOUNT_ID"),
                req_secret("api_token", "CLOUDFLARE_API_TOKEN"),
                opt("access_key_id", "AWS_ACCESS_KEY_ID"),
                opt_secret("secret_access_key", "AWS_SECRET_ACCESS_KEY"),
                opt("endpoint", "AWS_ENDPOINT_URL"),
                opt("bucket", "R2_BUCKET"),
                opt("zone_id", "CLOUDFLARE_ZONE_ID"),
                opt("email", "CLOUDFLARE_EMAIL"),
                opt_secret("global_api_key", "CLOUDFLARE_API_KEY"),
            ],
        },
        Provider {
            name: "aws".to_string(),
            title: "AWS".to_string(),
            fields: vec![
                req("access_key_id", "AWS_ACCESS_KEY_ID"),
                req_secret("secret_access_key", "AWS_SECRET_ACCESS_KEY"),
                opt_secret("session_token", "AWS_SESSION_TOKEN"),
                opt("region", "AWS_DEFAULT_REGION"),
            ],
        },
        Provider {
            name: "s3".to_string(),
            title: "S3".to_string(),
            fields: vec![
                req("access_key_id", "AWS_ACCESS_KEY_ID"),
                req_secret("secret_access_key", "AWS_SECRET_ACCESS_KEY"),
                req("endpoint", "AWS_ENDPOINT_URL"),
                opt("region", "AWS_REGION"),
                opt("bucket", "S3_BUCKET"),
            ],
        },
        Provider {
            name: "github".to_string(),
            title: "GitHub".to_string(),
            fields: vec![
                req_secret("token", "GITHUB_TOKEN"),
                opt("user", "GITHUB_USER"),
            ],
        },
        Provider {
            name: "gitea".to_string(),
            title: "Gitea".to_string(),
            fields: vec![
                req_secret("token", "GITEA_TOKEN"),
                req("url", "GITEA_URL"),
                opt("user", "GITEA_USER"),
            ],
        },
        Provider {
            name: "gitlab".to_string(),
            title: "GitLab".to_string(),
            fields: vec![
                req_secret("token", "GITLAB_TOKEN"),
                opt("url", "GITLAB_URL"),
            ],
        },
        Provider {
            name: "slack".to_string(),
            title: "Slack".to_string(),
            fields: vec![
                req_secret("bot_token", "SLACK_BOT_TOKEN"),
                opt_secret("app_token", "SLACK_APP_TOKEN"),
            ],
        },
        Provider {
            name: "digitalocean".to_string(),
            title: "DigitalOcean".to_string(),
            fields: vec![req_secret("token", "DIGITALOCEAN_TOKEN")],
        },
        Provider {
            name: "npm".to_string(),
            title: "npm".to_string(),
            fields: vec![req_secret("token", "NPM_TOKEN")],
        },
        Provider {
            name: "xai".to_string(),
            title: "xAI".to_string(),
            fields: vec![req_secret("api_key", "XAI_API_KEY")],
        },
        Provider {
            name: "sendgrid".to_string(),
            title: "SendGrid".to_string(),
            fields: vec![req_secret("api_key", "SENDGRID_API_KEY")],
        },
        Provider {
            name: "pypi".to_string(),
            title: "PyPI".to_string(),
            fields: vec![
                req_secret("token", "TWINE_PASSWORD"),
                opt("user", "TWINE_USERNAME"),
            ],
        },
        Provider {
            name: "anthropic".to_string(),
            title: "Anthropic".to_string(),
            fields: vec![req_secret("api_key", "ANTHROPIC_API_KEY")],
        },
        Provider {
            name: "openai".to_string(),
            title: "OpenAI".to_string(),
            fields: vec![
                req_secret("api_key", "OPENAI_API_KEY"),
                opt("org_id", "OPENAI_ORG_ID"),
            ],
        },
        Provider {
            name: "vault".to_string(),
            title: "Vault".to_string(),
            fields: vault_fields(),
        },
    ]
}

fn vault_fields() -> Vec<Field> {
    let mut fields = vec![
        req("addr", "VAULT_ADDR"),
        req_secret("role_id", "VAULT_ROLE_ID"),
        req_secret("secret_id", "VAULT_SECRET_ID"),
        opt_secret("root_token", "VAULT_TOKEN"),
    ];
    for i in 1..=5 {
        fields.push(opt_secret(
            &format!("share_{i}"),
            &format!("VAULT_UNSEAL_SHARE_{i}"),
        ));
    }
    fields.push(opt("mount", "VAULT_MOUNT"));
    fields.push(opt("prefix", "VAULT_PREFIX"));
    fields.push(opt("namespace", "VAULT_NAMESPACE"));
    fields
}

fn req(key: &str, env: &str) -> Field {
    Field {
        key: key.to_string(),
        secret: false,
        optional: false,
        env: env.to_string(),
    }
}

fn req_secret(key: &str, env: &str) -> Field {
    Field {
        key: key.to_string(),
        secret: true,
        optional: false,
        env: env.to_string(),
    }
}

fn opt(key: &str, env: &str) -> Field {
    Field {
        key: key.to_string(),
        secret: false,
        optional: true,
        env: env.to_string(),
    }
}

fn opt_secret(key: &str, env: &str) -> Field {
    Field {
        key: key.to_string(),
        secret: true,
        optional: true,
        env: env.to_string(),
    }
}
