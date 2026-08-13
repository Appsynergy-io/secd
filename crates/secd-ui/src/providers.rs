//! Built-in provider schemas for the add wizard (locked table).

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldSchema {
    pub key: &'static str,
    pub secret: bool,
    pub optional: bool,
    pub env: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSchema {
    pub name: &'static str,
    pub title: &'static str,
    pub fields: &'static [FieldSchema],
}

const fn req(key: &'static str, env: &'static str) -> FieldSchema {
    FieldSchema {
        key,
        secret: false,
        optional: false,
        env,
    }
}

const fn req_secret(key: &'static str, env: &'static str) -> FieldSchema {
    FieldSchema {
        key,
        secret: true,
        optional: false,
        env,
    }
}

const fn opt(key: &'static str, env: &'static str) -> FieldSchema {
    FieldSchema {
        key,
        secret: false,
        optional: true,
        env,
    }
}

const fn opt_secret(key: &'static str, env: &'static str) -> FieldSchema {
    FieldSchema {
        key,
        secret: true,
        optional: true,
        env,
    }
}

const CLOUDFLARE: &[FieldSchema] = &[
    req("account_id", "CLOUDFLARE_ACCOUNT_ID"),
    req_secret("api_token", "CLOUDFLARE_API_TOKEN"),
    opt("access_key_id", "AWS_ACCESS_KEY_ID"),
    opt_secret("secret_access_key", "AWS_SECRET_ACCESS_KEY"),
    opt("endpoint", "AWS_ENDPOINT_URL"),
    opt("bucket", "R2_BUCKET"),
    opt("zone_id", "CLOUDFLARE_ZONE_ID"),
    opt("email", "CLOUDFLARE_EMAIL"),
    opt_secret("global_api_key", "CLOUDFLARE_API_KEY"),
];

const AWS: &[FieldSchema] = &[
    req("access_key_id", "AWS_ACCESS_KEY_ID"),
    req_secret("secret_access_key", "AWS_SECRET_ACCESS_KEY"),
    opt_secret("session_token", "AWS_SESSION_TOKEN"),
    opt("region", "AWS_DEFAULT_REGION"),
];

const S3: &[FieldSchema] = &[
    req("access_key_id", "AWS_ACCESS_KEY_ID"),
    req_secret("secret_access_key", "AWS_SECRET_ACCESS_KEY"),
    req("endpoint", "AWS_ENDPOINT_URL"),
    opt("region", "AWS_REGION"),
    opt("bucket", "S3_BUCKET"),
];

const GITHUB: &[FieldSchema] = &[
    req_secret("token", "GITHUB_TOKEN"),
    opt("user", "GITHUB_USER"),
];

const GITEA: &[FieldSchema] = &[
    req_secret("token", "GITEA_TOKEN"),
    req("url", "GITEA_URL"),
    opt("user", "GITEA_USER"),
];

const GITLAB: &[FieldSchema] = &[
    req_secret("token", "GITLAB_TOKEN"),
    opt("url", "GITLAB_URL"),
];

const SLACK: &[FieldSchema] = &[
    req_secret("bot_token", "SLACK_BOT_TOKEN"),
    opt_secret("app_token", "SLACK_APP_TOKEN"),
];

const VAULT: &[FieldSchema] = &[
    req("addr", "VAULT_ADDR"),
    req_secret("role_id", "VAULT_ROLE_ID"),
    req_secret("secret_id", "VAULT_SECRET_ID"),
    opt_secret("root_token", "VAULT_TOKEN"),
    opt_secret("share_1", "VAULT_UNSEAL_SHARE_1"),
    opt_secret("share_2", "VAULT_UNSEAL_SHARE_2"),
    opt_secret("share_3", "VAULT_UNSEAL_SHARE_3"),
    opt_secret("share_4", "VAULT_UNSEAL_SHARE_4"),
    opt_secret("share_5", "VAULT_UNSEAL_SHARE_5"),
    opt("mount", "VAULT_MOUNT"),
    opt("prefix", "VAULT_PREFIX"),
    opt("namespace", "VAULT_NAMESPACE"),
];

pub const PROVIDERS: &[ProviderSchema] = &[
    ProviderSchema {
        name: "cloudflare",
        title: "Cloudflare",
        fields: CLOUDFLARE,
    },
    ProviderSchema {
        name: "aws",
        title: "AWS",
        fields: AWS,
    },
    ProviderSchema {
        name: "s3",
        title: "S3",
        fields: S3,
    },
    ProviderSchema {
        name: "github",
        title: "GitHub",
        fields: GITHUB,
    },
    ProviderSchema {
        name: "gitea",
        title: "Gitea",
        fields: GITEA,
    },
    ProviderSchema {
        name: "gitlab",
        title: "GitLab",
        fields: GITLAB,
    },
    ProviderSchema {
        name: "slack",
        title: "Slack",
        fields: SLACK,
    },
    ProviderSchema {
        name: "digitalocean",
        title: "DigitalOcean",
        fields: &[req_secret("token", "DIGITALOCEAN_TOKEN")],
    },
    ProviderSchema {
        name: "npm",
        title: "npm",
        fields: &[req_secret("token", "NPM_TOKEN")],
    },
    ProviderSchema {
        name: "xai",
        title: "xAI",
        fields: &[req_secret("api_key", "XAI_API_KEY")],
    },
    ProviderSchema {
        name: "sendgrid",
        title: "SendGrid",
        fields: &[req_secret("api_key", "SENDGRID_API_KEY")],
    },
    ProviderSchema {
        name: "pypi",
        title: "PyPI",
        fields: &[
            req_secret("token", "TWINE_PASSWORD"),
            opt("user", "TWINE_USERNAME"),
        ],
    },
    ProviderSchema {
        name: "anthropic",
        title: "Anthropic",
        fields: &[req_secret("api_key", "ANTHROPIC_API_KEY")],
    },
    ProviderSchema {
        name: "openai",
        title: "OpenAI",
        fields: &[
            req_secret("api_key", "OPENAI_API_KEY"),
            opt("org_id", "OPENAI_ORG_ID"),
        ],
    },
    ProviderSchema {
        name: "vault",
        title: "Vault",
        fields: VAULT,
    },
];

pub fn provider_by_name(name: &str) -> Option<&'static ProviderSchema> {
    PROVIDERS.iter().find(|p| p.name == name)
}
