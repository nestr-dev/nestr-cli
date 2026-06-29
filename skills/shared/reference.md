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
- `-w, --workspace <id>` — act on a specific workspace, overriding the profile's (for full-account profiles).
- `-o, --output text|json` — `text` (tables) or `json` (raw, for `jq`).
- `--yes` — skip destructive-action confirmations (required for agents/scripts).
- `--read-only` — hard-block every write (also `NESTR_READ_ONLY=1`).

## Workspace context

A profile pins **one active workspace**. Some commands act on that workspace; the rest
work by nest id or are account-/user-level and ignore it.

```bash
nestr workspaces list                    # the workspaces this identity can reach
nestr workspaces use <id>                # persist a different active workspace on the profile
```

For a single command, override with `-w, --workspace <id>` instead of switching. A
workspace-scoped command with no active workspace errors with guidance to run
`nestr workspaces use <id>` or pass `-w`.

- **Need an active workspace:** `search` (workspace-wide), `projects`, `work`, `circles`,
  `roles`, `users`, `groups`, `labels list`/`labels get`, `insights`, `export`,
  `webhooks`, `workspaces apps`, `nests bulk-reorder`.
- **Don't (work by id, or account/user-level):** `nests get`/`create`/`update`/`delete`,
  `comments`, `inbox`, `plan`, `notifications`, `tensions`, `links`, `me`, `auth`,
  `profiles`, `workspaces list`/`get`/`use`/`create`.

**Search is workspace-scoped.** Results only come from the active workspace (or the
`--in <nestId>` subtree) — there is no cross-workspace search endpoint. If a search
returns nothing, the nest may simply live in **another** workspace: list them with
`nestr workspaces list`, then **ask the user** before switching
(`nestr workspaces use <id>`, or a one-off `-w <id>`) and searching again.

## The model

Everything in Nestr is a **Nest**. What a nest *is* is set by exactly **one prime
label** (see below): circles, roles, projects, goals, and tensions are all nests
carrying their prime label, and a nest with no prime label is a plain todo. Comments
and inbox items are lighter nests of their own. Responses come in a few shapes
(`{status, data, …}`, a bare array, or a bare object); `-o json` always prints the
raw unwrapped data.

Most list commands paginate with `--limit` and `--page` (page-based); pass `--page N`
to fetch the next page when the footer shows more. (`notifications list` is the
exception — it pages with `--limit`/`--skip`.)

## What a nest is

What a nest *is* is set by exactly **one prime label**; the CLI rejects two or more
("A nest can have only one prime label"). A nest with **no** prime label is a plain
todo. The 11 prime label codes — pass these to `--label`:

| code | what it is |
|---|---|
| `project` | a project (holds child todos/subtasks) |
| `goal` | a goal / objective |
| `result` | a key result |
| `metric` | a tracked metric |
| `checklist` | a (recurring) checklist |
| `meeting` | a meeting |
| `feedback` | a feedback item |
| `circle` | an organizational circle |
| `role` | a role within a circle |
| `anchor-circle` | the top-level anchor circle |
| `tension` | a governance tension |

Other labels (e.g. `urgent`, `now`) are free-form, not prime — discover them with
`nestr labels list`. `now` is the label `plan add`/`plan remove` toggle.

## Purpose vs description

Two distinct fields on nests, circles, roles, and tension parts:

- **`--purpose`** — a single-line statement of *why* the thing exists. Central for
  circles and roles, optional-but-encouraged for a project. Purpose is **inherited**:
  a nest with no purpose of its own shows its parent's. Keep it to one line.
- **`--description`** — the *body*: the actual details/content.
- **`--due`** — a date, ISO format (e.g. `2026-07-01`).

Never put body text in `--purpose` — that is the classic Nestr mistake.

## Assignment

A nest's **assigned people** live in its `users` array (a list of user ids). For a
**project or task** this is who does the work; for a **role** it is who fills
(energizes) the role. A project created with no assignee shows up under nobody's work —
so assign it.

```bash
nestr nests create --title "Grow the disk" --parent <id> --label project --assignee me
nestr nests update <id> --assignee <userId>          # reassign (replaces the whole set)
```

- `--assignee` is repeatable and takes a **user id** (find ids with `nestr users list`),
  or the literal `me` for the authenticated user (resolved via `nestr me`).
- On `nests update` it **replaces** the assigned set — re-list anyone you want to keep.

## Linking to a nest

The canonical web permalink for a nest is **`{host}/n/{nestId}`** — the path segment is
`/n/`, never `/nest/`, `/nests/`, or a `#/` hash route. With a known parent it is
`{host}/n/{parentId}/{nestId}` (opens the nest in context). `nests get`/`create`/`update`
print this as a `url:` line in `-o text`; don't hand-build links from memory.

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
