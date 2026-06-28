# nestr-cli

A fast, composable command-line interface for [Nestr](https://nestr.io) — built for terminal users and AI agents.

`nestr` puts the whole Nestr surface in your shell: auth & profiles, the everyday loop (search, nests, comments, inbox, plan, work), org & people, governance tensions, graph links, insights, export, and webhooks. Every command speaks `-o json` for piping to `jq`, honours `--read-only`, and runs non-interactively under `--yes` for scripts and agents.

## Install

### Quick install (macOS / Linux)

```sh
curl -fsSL https://raw.githubusercontent.com/nestr-dev/nestr-cli/main/install.sh | sh
```

Downloads the latest release for your platform, verifies its SHA-256 checksum (and, if `cosign` is installed, its keyless signature), and installs `nestr` to `/usr/local/bin` (falling back to `~/.local/bin`). Override with `NESTR_VERSION=0.1.0` (the bare version, no `v` prefix) or `NESTR_INSTALL_DIR=/path/to/bin`.

### Homebrew (macOS / Linux)

```sh
brew install nestr-dev/tap/nestr-cli
```

Installs the prebuilt, cosign-signed release binary (the installed command is `nestr`). Upgrade with `brew upgrade nestr-cli`.

### Agent skills (Claude Code, Cursor, Copilot, …)

Teach your coding agent the Nestr workflows — install the bundled [skills](skills/) with one command:

```sh
npx skills add nestr-dev/nestr-cli                          # choose skills interactively
npx skills add nestr-dev/nestr-cli --all -a claude-code -y  # all five, into Claude Code
```

Powered by [`skills`](https://github.com/vercel-labs/skills) (works across 40+ agents). The skills drive the `nestr` CLI, so install it (above) too.

### From source

```sh
cargo install --path .
nestr --help
```

### Prebuilt binaries

Download an archive for your platform from the [releases page](https://github.com/nestr-dev/nestr-cli/releases). Every release ships:

- `nestr-<version>-<target>.tar.gz` (`.zip` on Windows) for five targets — macOS (Intel + Apple Silicon), Linux musl (x86_64 + aarch64), Windows x86_64;
- `checksums-sha256.txt` plus a keyless [cosign](https://github.com/sigstore/cosign) signing bundle (`checksums-sha256.txt.sigstore.json`, signature + certificate + transparency-log entry in one file);
- a CycloneDX SBOM (`nestr-sbom.cyclonedx.json`) — its own checksum is listed in `checksums-sha256.txt`, so the bundle below covers it too.

Verify before running (optional but encouraged) — check the checksum, then confirm the checksums file was signed by this repo's release workflow (needs cosign v3+):

```sh
# 1. Verify checksums (Linux). On macOS: shasum -a 256 -c checksums-sha256.txt --ignore-missing
sha256sum -c checksums-sha256.txt --ignore-missing

# 2. Verify the signing bundle on the checksums file
cosign verify-blob \
  --bundle checksums-sha256.txt.sigstore.json \
  --certificate-identity-regexp '^https://github\.com/nestr-dev/nestr-cli/\.github/workflows/release\.yml@refs/tags/v' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  checksums-sha256.txt
```

## Quick start

```sh
nestr profiles add            # configure a profile (OAuth login or API key)
nestr me                      # verify authentication + show your identity
```

A **profile = a workspace + an identity**. Add several (`prod`, `staging`, `local`) and switch with `nestr profiles use <name>`.

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
| `-p, --profile <name>` | pick a profile for this invocation |
| `--api-key <key>` / `--host <url>` | override the profile's credential / host. Prefer `NESTR_API_KEY` — `--api-key` is visible in shell history and `ps`. |
| `-o, --output text\|json` | `text` tables (default) or raw `json` for `jq` |
| `--yes` | skip destructive-action confirmations (required for agents/scripts) |
| `--read-only` | hard-block every write (also `NESTR_READ_ONLY=1`) |

Env overrides (precedence: flags > env > profile > defaults): `NESTR_PROFILE`, `NESTR_API_KEY`, `NESTR_HOST`. Credentials live in the OS keyring, or a `0600` file on platforms without one (e.g. Linux musl builds).

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
