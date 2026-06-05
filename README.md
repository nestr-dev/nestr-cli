# nestr-cli

A fast, composable command-line interface for [Nestr](https://nestr.io) — built for terminal users and AI agents.

`nestr` puts the whole Nestr surface in your shell: auth & profiles, the everyday loop (search, nests, comments, inbox, plan, work), org & people, governance tensions, graph links, insights, export, and webhooks. Every command speaks `-o json` for piping to `jq`, honours `--read-only`, and runs non-interactively under `--yes` for scripts and agents.

## Install

### Quick install (macOS / Linux)

```sh
curl -fsSL https://raw.githubusercontent.com/nestr/nestr-cli/main/install.sh | sh
```

Downloads the latest signed release for your platform, verifies its SHA-256 checksum, and installs `nestr` to `/usr/local/bin` (falling back to `~/.local/bin`). Override with `NESTR_VERSION=0.1.0` or `NESTR_INSTALL_DIR=/path/to/bin`.

### From source

```sh
cargo install --path .
nestr --help
```

### Prebuilt binaries

Download an archive for your platform from the [releases page](https://github.com/nestr/nestr-cli/releases). Every release ships:

- `nestr-<version>-<target>.tar.gz` (`.zip` on Windows) for five targets — macOS (Intel + Apple Silicon), Linux musl (x86_64 + aarch64), Windows x86_64;
- `checksums-sha256.txt` plus a keyless [cosign](https://github.com/sigstore/cosign) signature (`.sig`) and certificate (`.pem`);
- a CycloneDX SBOM (`nestr-sbom.cyclonedx.json`).

Verify before running (optional but encouraged):

```sh
sha256sum -c checksums-sha256.txt --ignore-missing
cosign verify-blob \
  --certificate checksums-sha256.txt.pem \
  --signature checksums-sha256.txt.sig \
  --certificate-identity-regexp '^https://github.com/nestr/nestr-cli' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  checksums-sha256.txt
```

## Quick start

```sh
nestr profiles add            # configure a profile (OAuth login or API key)
nestr me                      # verify authentication + show your identity
```

A **profile = a workspace + an identity**. Add several (`prod`, `staging`, `local`), switch with `nestr profiles use <name>`, or fan a read-only query out across all of them with repeated `-p`.

Then the everyday loop:

```sh
nestr search "spec"           # find nests across the workspace
nestr nests get <id>          # read a nest (add -o json to pipe to jq)
nestr inbox create "Follow up with supplier"
nestr plan today              # what's on today's plan
nestr work                    # open projects + todos
```

## Global flags

| Flag | Purpose |
|---|---|
| `-p, --profile <name>` | pick a profile; repeat to fan out across workspaces |
| `--api-key <key>` / `--host <url>` | override the profile's credential / host |
| `-o, --output text\|json` | `text` tables (default) or raw `json` for `jq` |
| `--yes` | skip destructive-action confirmations (required for agents/scripts) |
| `--read-only` | hard-block every write (also `NESTR_READ_ONLY=1`) |

Env overrides (precedence: flags > env > profile > defaults): `NESTR_PROFILE`, `NESTR_API_KEY`, `NESTR_HOST`, `NESTR_WORKSPACE`. Credentials live in the OS keyring, or a `0600` file on platforms without one (e.g. Linux musl builds).

## Commands

| Group | What it does |
|---|---|
| `auth`, `profiles`, `me` | authenticate, switch profiles, show identity |
| `search`, `nests`, `comments` | find and read/edit nests; discuss |
| `inbox`, `plan`, `work` | capture, plan the day, open work |
| `notifications`, `labels`, `projects` | stay current, manage labels, list projects |
| `tensions` | governance: propose → consent → enact |
| `workspaces`, `circles`, `roles`, `users`, `groups` | org structure & members |
| `links`, `insights`, `export` | graph links, metrics, JSON dumps |
| `webhooks` | workspace event subscriptions |

Run `nestr <group> --help` for subcommands. A fuller command map with per-group notes lives in [`skills/shared/reference.md`](skills/shared/reference.md).

## Skills

Five `SKILL.md` files under [`skills/`](skills/README.md) document the CLI for agents (and humans), authored as plain Markdown so the hosted Nestr MCP can consume them over time:

- **`nestr-basics`** — the everyday loop (search, nests, inbox, plan, comments).
- **`nestr-governance`** — tensions: propose, review changes, consent, vote.
- **`nestr-org`** — workspaces, circles, roles, users, groups.
- **`nestr-insights`** — graph links, insights/metrics, and JSON export.
- **`nestr-webhooks`** — workspace event subscriptions.

Start at [`skills/README.md`](skills/README.md); shared setup and global flags are in [`skills/shared/reference.md`](skills/shared/reference.md).

## License

Apache-2.0. Copyright 2026 Nestr. Derived from [coralogix/cx-cli](https://github.com/coralogix/cx-cli) (Apache-2.0); see [`NOTICE`](NOTICE).
