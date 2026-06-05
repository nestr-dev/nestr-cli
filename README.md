# nestr-cli

A fast, composable command-line interface for [Nestr](https://nestr.io), for terminal users and AI agents.

> Status: feature-complete across the API surface — auth & profiles, the everyday loop (search, nests, comments, inbox, plan, notifications, labels, projects, work), org & people (workspaces, circles, roles, users, groups), governance tensions (propose → consent → enact), graph links, insights, export, and webhooks. See `docs/superpowers/specs/` for the design and roadmap.

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

Five `SKILL.md` files under `skills/` cover the CLI, authored as plain Markdown so
the hosted Nestr MCP can consume them over time:

- **`nestr-basics`** — the everyday loop (search, nests, inbox, plan, comments).
- **`nestr-governance`** — tensions: propose, review changes, consent, vote.
- **`nestr-org`** — workspaces, circles, roles, users, groups (admin/structure).
- **`nestr-insights`** — graph links, insights/metrics, and JSON export.
- **`nestr-webhooks`** — workspace event subscriptions.

Start at [`skills/README.md`](skills/README.md); shared setup, global flags, and a
full command map live in [`skills/shared/reference.md`](skills/shared/reference.md).

## License

Apache-2.0. Derived from [coralogix/cx-cli](https://github.com/coralogix/cx-cli) (Apache-2.0); see `NOTICE`.
