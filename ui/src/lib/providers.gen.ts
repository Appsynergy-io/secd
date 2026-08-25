/** Generated from crates/secd-core/src/provider.rs. Do not edit. */

export type ProviderField = {
  key: string;
  secret: boolean;
  optional: boolean;
  env: string;
};

export type Provider = {
  name: string;
  title: string;
  fields: readonly ProviderField[];
};

export const PROVIDERS: readonly Provider[] = [
  {
    name: "cloudflare",
    title: "Cloudflare",
    fields: [
      { key: "account_id", secret: false, optional: false, env: "CLOUDFLARE_ACCOUNT_ID" },
      { key: "api_token", secret: true, optional: false, env: "CLOUDFLARE_API_TOKEN" },
      { key: "access_key_id", secret: false, optional: true, env: "AWS_ACCESS_KEY_ID" },
      { key: "secret_access_key", secret: true, optional: true, env: "AWS_SECRET_ACCESS_KEY" },
      { key: "endpoint", secret: false, optional: true, env: "AWS_ENDPOINT_URL" },
      { key: "bucket", secret: false, optional: true, env: "R2_BUCKET" },
      { key: "zone_id", secret: false, optional: true, env: "CLOUDFLARE_ZONE_ID" },
      { key: "email", secret: false, optional: true, env: "CLOUDFLARE_EMAIL" },
      { key: "global_api_key", secret: true, optional: true, env: "CLOUDFLARE_API_KEY" },
    ],
  },
  {
    name: "aws",
    title: "AWS",
    fields: [
      { key: "access_key_id", secret: false, optional: false, env: "AWS_ACCESS_KEY_ID" },
      { key: "secret_access_key", secret: true, optional: false, env: "AWS_SECRET_ACCESS_KEY" },
      { key: "session_token", secret: true, optional: true, env: "AWS_SESSION_TOKEN" },
      { key: "region", secret: false, optional: true, env: "AWS_DEFAULT_REGION" },
    ],
  },
  {
    name: "s3",
    title: "S3",
    fields: [
      { key: "access_key_id", secret: false, optional: false, env: "AWS_ACCESS_KEY_ID" },
      { key: "secret_access_key", secret: true, optional: false, env: "AWS_SECRET_ACCESS_KEY" },
      { key: "endpoint", secret: false, optional: false, env: "AWS_ENDPOINT_URL" },
      { key: "region", secret: false, optional: true, env: "AWS_REGION" },
      { key: "bucket", secret: false, optional: true, env: "S3_BUCKET" },
    ],
  },
  {
    name: "github",
    title: "GitHub",
    fields: [
      { key: "token", secret: true, optional: false, env: "GITHUB_TOKEN" },
      { key: "user", secret: false, optional: true, env: "GITHUB_USER" },
    ],
  },
  {
    name: "gitea",
    title: "Gitea",
    fields: [
      { key: "token", secret: true, optional: false, env: "GITEA_TOKEN" },
      { key: "url", secret: false, optional: false, env: "GITEA_URL" },
      { key: "user", secret: false, optional: true, env: "GITEA_USER" },
    ],
  },
  {
    name: "gitlab",
    title: "GitLab",
    fields: [
      { key: "token", secret: true, optional: false, env: "GITLAB_TOKEN" },
      { key: "url", secret: false, optional: true, env: "GITLAB_URL" },
    ],
  },
  {
    name: "slack",
    title: "Slack",
    fields: [
      { key: "bot_token", secret: true, optional: false, env: "SLACK_BOT_TOKEN" },
      { key: "app_token", secret: true, optional: true, env: "SLACK_APP_TOKEN" },
    ],
  },
  {
    name: "digitalocean",
    title: "DigitalOcean",
    fields: [
      { key: "token", secret: true, optional: false, env: "DIGITALOCEAN_TOKEN" },
    ],
  },
  {
    name: "npm",
    title: "npm",
    fields: [
      { key: "token", secret: true, optional: false, env: "NPM_TOKEN" },
    ],
  },
  {
    name: "xai",
    title: "xAI",
    fields: [
      { key: "api_key", secret: true, optional: false, env: "XAI_API_KEY" },
    ],
  },
  {
    name: "sendgrid",
    title: "SendGrid",
    fields: [
      { key: "api_key", secret: true, optional: false, env: "SENDGRID_API_KEY" },
    ],
  },
  {
    name: "pypi",
    title: "PyPI",
    fields: [
      { key: "token", secret: true, optional: false, env: "TWINE_PASSWORD" },
      { key: "user", secret: false, optional: true, env: "TWINE_USERNAME" },
    ],
  },
  {
    name: "anthropic",
    title: "Anthropic",
    fields: [
      { key: "api_key", secret: true, optional: false, env: "ANTHROPIC_API_KEY" },
    ],
  },
  {
    name: "openai",
    title: "OpenAI",
    fields: [
      { key: "api_key", secret: true, optional: false, env: "OPENAI_API_KEY" },
      { key: "org_id", secret: false, optional: true, env: "OPENAI_ORG_ID" },
    ],
  },
  {
    name: "vault",
    title: "Vault",
    fields: [
      { key: "addr", secret: false, optional: false, env: "VAULT_ADDR" },
      { key: "role_id", secret: true, optional: false, env: "VAULT_ROLE_ID" },
      { key: "secret_id", secret: true, optional: false, env: "VAULT_SECRET_ID" },
      { key: "root_token", secret: true, optional: true, env: "VAULT_TOKEN" },
      { key: "share_1", secret: true, optional: true, env: "VAULT_UNSEAL_SHARE_1" },
      { key: "share_2", secret: true, optional: true, env: "VAULT_UNSEAL_SHARE_2" },
      { key: "share_3", secret: true, optional: true, env: "VAULT_UNSEAL_SHARE_3" },
      { key: "share_4", secret: true, optional: true, env: "VAULT_UNSEAL_SHARE_4" },
      { key: "share_5", secret: true, optional: true, env: "VAULT_UNSEAL_SHARE_5" },
      { key: "mount", secret: false, optional: true, env: "VAULT_MOUNT" },
      { key: "prefix", secret: false, optional: true, env: "VAULT_PREFIX" },
      { key: "namespace", secret: false, optional: true, env: "VAULT_NAMESPACE" },
    ],
  },
];
