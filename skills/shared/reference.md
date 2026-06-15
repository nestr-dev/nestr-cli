# nestr CLI — Shared Reference

Cross-cutting setup and conventions for every `nestr` skill.

## Setup

```bash
nestr profiles add                       # interactive: OAuth login or paste an API key
nestr profiles list                      # configured profiles
nestr profiles use <name>                # set the default profile
nestr profiles remove <name>             # delete a profile + its credentials
nestr auth login [profile]               # (re)run the browser OAuth login
nestr auth status [profile]              # show resolved profile + token validity
nestr auth logout [profile]              # invalidate + clear local credentials
nestr me                                 # verify auth + show the active identity
```

A **profile = a workspace + an identity**. The profile's `host` selects the
environment (prod / staging / local), so `prod`, `staging`, and `local` profiles
sit side by side. Credentials live in the OS keyring or a `0600` file.

**Env overrides** (precedence: CLI flags > env > profile > defaults):
`NESTR_PROFILE`, `NESTR_API_KEY`, `NESTR_HOST`.

## Global flags

- `-p, --profile <name>` — pick a profile for this invocation.
- `--api-key <key>` / `--host <url>` — override the profile's credential / host.
- `-o, --output text|json` — `text` (tables) or `json` (raw, for `jq`).
- `--yes` — skip destructive-action confirmations (required for agents/scripts).
- `--read-only` — hard-block every write (also `NESTR_READ_ONLY=1`).

## The model

Everything in Nestr is a **Nest** — circles, roles, projects, todos, tensions,
inbox items, and comments are all nests distinguished by labels. Responses come in
a few shapes (`{status, data, …}`, a bare array, or a bare object); `-o json`
always prints the raw unwrapped data.

List commands paginate with `--limit` and `--page` (page-based); pass `--page N` to fetch the next page when the footer shows more are available.

## Command map

| Group | What it does | Skill |
|---|---|---|
| `auth`, `profiles`, `me` | authenticate, switch profiles, identity | this reference |
| `search` | find nests across a workspace or subtree | nestr-basics |
| `nests` | read/create/update/delete/reorder nests + labels | nestr-basics |
| `comments` | discuss on a nest | nestr-basics |
| `inbox`, `plan`, `work` | capture, plan the day, open work | nestr-basics |
| `notifications`, `labels`, `projects` | stay current, labels, projects | nestr-basics |
| `tensions` | governance: propose → consent → enact | nestr-governance |
| `workspaces`, `circles`, `roles`, `users`, `groups` | org structure & members | nestr-org |
| `links`, `insights`, `export` | graph links, metrics, JSON dumps | nestr-insights |
| `webhooks` | workspace event subscriptions | nestr-webhooks |
