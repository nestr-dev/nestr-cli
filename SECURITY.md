# Security Policy

## Supported versions

Only the latest released version of `nestr-cli` receives security fixes.

## Reporting a vulnerability

**Please do not open a public issue for security-sensitive reports.**

Report privately via GitHub: go to the repository's **Security** tab →
**Report a vulnerability** (private security advisory). This reaches the
maintainers without disclosing the issue publicly.

We aim to acknowledge reports within a few business days and will keep you
updated on remediation and disclosure timing.

## Scope

`nestr-cli` stores OAuth tokens / API keys via the OS keyring (file fallback,
`0600`) and talks to a Nestr host over TLS. Reports about credential exposure,
token handling, TLS, or the release/`install.sh` supply chain are especially
welcome.
