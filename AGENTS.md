# AGENTS.md

Guidance for AI agents and contributors working **on** nestr-cli. End-user usage
(install, commands, flags) is in [README.md](README.md); the full command map is
[skills/shared/reference.md](skills/shared/reference.md). This file covers what
those don't: the dev loop, conventions, and gotchas.

## What this is

A Rust CLI (binary `nestr`, library crate `nestr_cli`) over the Nestr REST API — a
fast, scriptable companion to the hosted Nestr MCP. Covers the everyday loop, org &
people, governance tensions, graph links, insights, export, and webhooks. (The parked
`agents` command group and `replaceNest` are intentionally not yet wired up.)

## Dev loop

Toolchain is pinned in `rust-toolchain.toml` (1.94.1 + clippy + rustfmt). Before pushing:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

CI (`.github/workflows/{test,lint}.yml`) runs on PRs to `main`: tests on
Linux/macOS/Windows, lint (fmt + clippy + cargo-audit) on Linux. Tests are hermetic —
integration tests under `tests/<group>.rs` mock the API with `wiremock`; no live server
is needed.

## Gotcha: doc commands are tested

`tests/skill_accuracy.rs` extracts every `nestr …` command from the ` ```bash `/
` ```sh `/ ` ```shell ` fences in `skills/**/*.md` **and** `README.md`, then runs
`nestr <subcommand-path> --help`. Any command example you put in a doc must resolve
to a real subcommand path or `cargo test` fails. Non-`nestr` lines (curl, cargo,
cosign, …) are ignored.

## Layout

- `src/main.rs` — clap (derive) CLI definition + dispatch. `src/lib.rs` — exports.
- `src/commands/<group>.rs` — one module per command group, declared in
  `src/commands/mod.rs`. Shared `GlobalArgs` and `resolve_client()` live in `mod.rs`.
- `src/api_client.rs` — reqwest wrapper; OAuth profiles get reactive 401-refresh via
  `with_refresh` (bearer in a shared cell). `src/oauth.rs` — PKCE login.
  `src/keyring_store.rs` — OS keyring (file fallback on musl, which has no backend).
- `src/config.rs` — TOML profiles under `~/.nestr` (`config.toml` +
  `profiles/<name>.toml`; `NESTR_HOME` overrides the base dir).
- `src/render.rs` — text (`tabled`/`colored`) + json output. `src/views.rs` — `*View`
  structs. `src/safety.rs` — write confirmation, `--read-only`/`NESTR_READ_ONLY`,
  agent-mode detection. `src/validation.rs`, `src/error.rs`.

## Conventions

- **API envelopes are inconsistent.** Responses arrive as `{status,data}`, a bare
  array, or a bare object — deserialize per-endpoint and unwrap. `-o json` always
  prints the unwrapped data; keep both `-o text` and `-o json` working for every command.
- **Profiles = workspace + identity.** Resolution precedence: CLI flags > env
  (`NESTR_PROFILE` / `NESTR_API_KEY` / `NESTR_HOST`) > profile > defaults.
- **Writes go through `safety::confirm`** and must honour `--read-only`; in
  agent/non-interactive mode they require `--yes`.
- **Responses are buffered fully in memory** (no size cap) before parsing/render.
  Fine for a short-lived CLI against a trusted host; a hostile or misconfigured server
  could make a single invocation allocate large amounts. Known limitation.

## Adding a command group

1. `src/commands/<group>.rs` — `fetch_*`/action fns taking a `&NestrClient` (keep them
   pure and testable) plus the command runner.
2. Declare the module in `src/commands/mod.rs` and add the subcommand in `src/main.rs`.
3. Add `*View` structs in `views.rs` and a renderer in `render.rs`.
4. `tests/<group>.rs` — wiremock tests asserting the unwrap/mapping for each response shape.
5. Update the matching `skills/*/SKILL.md` and `skills/shared/reference.md`, and keep
   `skill_accuracy` green.

## Release

`.github/workflows/release.yml` is tag-triggered (`v*`): the `verify-version` job
requires the tag to equal `Cargo.toml`'s `version`. It builds 5 targets and publishes
SHA-256 checksums + a keyless cosign signature + a CycloneDX SBOM. `install.sh` is the
artifact-naming contract — rename an archive in one, change the other. crates.io,
Homebrew, nix, and shell completions are intentionally **not** wired up.

## Specs

Design docs and phase plans are **not in this repo** — they live in the private sibling
checkout `../nestr-cli-specs` (`specs/`, `plans/`). Read them for rationale before large changes.

## Licensing & commits

Apache-2.0, `Copyright 2026 Nestr`, derived from
[coralogix/cx-cli](https://github.com/coralogix/cx-cli) (see `NOTICE`); no per-file
headers. **Commit messages and PR descriptions carry no AI attribution** — no
"Co-Authored-By", no "Generated with …".
