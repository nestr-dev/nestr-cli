---
name: nestr-org
description: Use when administering a Nestr organization from the terminal — workspaces and their apps, circles, roles, members, and groups. The admin/structure side of the `nestr` CLI.
---

# Nestr Org & People

`nestr` administers your Nestr organization's structure. A **workspace** holds
**circles** (nested teams); each circle holds **roles**; **users** are members who
fill roles and belong to **groups**. Most writes here are **admin-only** — a
non-admin token gets a 403.

First run `nestr profiles add` (OAuth or API key) — a profile pairs a **workspace**
with an **identity**. Global flags on every command: `-p/--profile`, `-o text|json`,
`--read-only` (block writes), and `--yes` (skip write confirmations; required for
agents). Full setup, env overrides, and the command map:
[shared/reference.md](https://github.com/nestr-dev/nestr-cli/blob/main/skills/shared/reference.md).

## Find the structure

```bash
nestr workspaces list                    # your workspaces
nestr workspaces get <id>                # one workspace
nestr workspaces use <id>                # pin this profile's active workspace
nestr circles list                       # circles in the active workspace
nestr circles get <id>                   # one circle (purpose, accountabilities, domains)
nestr roles list                         # all roles in the workspace
nestr roles get <id>                     # one role
nestr users list                         # workspace members
nestr users get <id>                     # one user
nestr groups list                        # groups
nestr groups get <id>                    # one group (by id or name)
```

Look inside a circle:

```bash
nestr circles roles <id>                 # roles in the circle
nestr circles projects <id>              # the circle's projects
nestr circles tensions <id>              # the circle's open tensions
nestr circles posts <id>                 # the circle's comments
```

## Shape the structure (admin)

```bash
nestr workspaces create --title "Acme" --governance holacracy \
  --plan starter --collaborators collaborate --app insights   # bootstrap a workspace (user-scoped token)

nestr circles create --title "Marketing" --purpose "Grow the audience"
nestr circles create --title "Content" --parent <circleId>      # a sub-circle
nestr circles update <id> --purpose "Revised purpose" \
  --accountability "Owning the editorial calendar"

nestr roles create --parent <circleId> --title "Editor" \
  --purpose "Quality of published content" --accountability "Approving drafts"
nestr roles update <id> --purpose "Revised purpose"
```

`circles update` and `roles update` **replace** accountabilities/domains when you
pass them — send the full set, not a delta.

## Manage members (admin)

```bash
nestr users add person@example.com --full-name "New Hire"
nestr users update <id> --email new@example.com
nestr users roles <id>                   # roles a user fills
nestr users groups show <id>             # a user's groups
nestr users groups set <id> editors leads    # replace their groups
nestr groups create "editors"
```

## Workspace apps

```bash
nestr workspaces apps                     # show which apps are on
nestr workspaces apps set insights on     # enable an app (okr | feedback | insights)
```

## Safety

Admin writes (`circles`/`roles` `create`/`update`, `users add`/`update`,
`users groups set`, `groups create`, `workspaces apps set`) are blocked by
`--read-only`. Add `-o json` to any command to pipe structured data to `jq`.
