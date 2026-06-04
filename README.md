# nestr-cli

A fast, composable command-line interface for [Nestr](https://nestr.io), for terminal users and AI agents.

> Status: Phase 3 (governance) — auth/profiles, the everyday loop, org & people, and governance tensions (propose → consent → enact). See `docs/superpowers/specs/` for the design and roadmap.

## Install (from source)

```bash
cargo install --path .
nestr --help
```

## Quick start

```bash
nestr profiles add            # configure a profile (OAuth or API key)
nestr me                      # verify authentication
```

Then the everyday loop:

```bash
nestr search "spec"           # find nests across the workspace
nestr nests get <id>          # read a nest (add -o json to pipe to jq)
nestr inbox create "Follow up with supplier"
nestr plan today              # what's on today's plan
nestr work                    # open projects + todos
```

Add `-o json` to any command for raw JSON, `--read-only` to block writes, and
`--yes` to skip destructive-action confirmations in scripts/agents.

## Skills

This repo ships a `nestr-basics` skill (`skills/nestr-basics/SKILL.md`) covering
the everyday command loop. It is authored as a plain `SKILL.md` so the hosted
Nestr MCP can consume the same file over time.

A `nestr-governance` skill (`skills/nestr-governance/SKILL.md`) covers the tension
workflow: propose, review changes, submit for consent, and vote.

## License

Apache-2.0. Derived from [coralogix/cx-cli](https://github.com/coralogix/cx-cli) (Apache-2.0); see `NOTICE`.
