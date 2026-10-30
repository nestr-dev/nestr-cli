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
  A read-only Nestr API key is refused server-side on every write; the CLI reports
  that as `this API key is read-only` rather than a scope problem. Pair such a key
  with `--read-only` to stop before the request is sent.

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
  `webhooks`, `workspaces apps`, `workspaces docs`, `nests bulk-reorder`, `connectors`,
  `agents` (except `agents mode`), and `hints` with no nest id.
- **Don't (work by id, or account/user-level):** `nests get`/`create`/`update`/`delete`,
  `nests files`, `comments`, `inbox`, `plan`, `notifications`, `dm`, `queues`, `tensions`,
  `links`, `activity`, `me`, `auth`, `profiles`, `workspaces list`/`get`/`use`/`create`,
  `agents mode`.

**Search is workspace-scoped.** Results only come from the active workspace (or the
`--in <nestId>` subtree) — there is no cross-workspace search endpoint. If a search
returns nothing, the nest may simply live in **another** workspace: list them with
`nestr workspaces list`, then **ask the user** before switching
(`nestr workspaces use <id>`, or a one-off `-w <id>`) and searching again.

## The model

Everything in Nestr is a **Nest**. What a nest *is* is set by exactly **one prime
label** (see below): circles, roles, projects, goals, and tensions are all nests
carrying their prime label, and a nest with no prime label is a plain todo (the
server attaches the non-prime `task` label to it automatically). Comments
and inbox items are lighter nests of their own. Responses come in a few shapes
(`{status, data, …}`, a bare array, or a bare object); `-o json` always prints the
raw unwrapped data.

**Warnings go to stderr.** When the API says it ignored something or narrowed a
query, the CLI prints that to **stderr** and leaves stdout as the unwrapped data:
parts of a write it skipped (`warnings`, e.g. a read-only or deprecated `--field`), dropped search
operators (`searchWarnings`), defaults it added (`appliedDefaults`, e.g. the
`depth:1` that `nests children --search` gets), and response-level `hints` (e.g.
unread posts). In text mode each prints as a `warning:` / `note:` / `hint:` line; in
`-o json` mode each response's notices print as one compact JSON object on stderr,
keyed as the API sends them (`{"warnings":[…]}`). Read stderr after a write: a
`warning:` there means part of the request was not saved.

Most list commands paginate with `--limit` and `--page` (page-based); pass `--page N`
to fetch the next page when the footer shows more. (`notifications list` is the
exception — it pages with `--limit`/`--skip`.)

## What a nest is

What a nest *is* is set by exactly **one prime label**; the CLI rejects two or more
("A nest can have only one prime label"). A nest with **no** prime label is a plain
todo: the server attaches the non-prime `task` label to every completable nest
automatically, so pass `--label task` when creating one to be explicit, and use
`label:task` in search to find todos. The 15 prime label codes — pass these to
`--label`:

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
| `userstory` | a Scrum user story (also carries `project` — see below) |
| `sprint` | a Scrum sprint |
| `epic` | a Scrum epic |
| `milestone` | a Scrum milestone |

**One exception to the one-prime rule:** `userstory` **implies** `project` — every
story is also a project, so it inherits status, completion, and workspace-wide
reporting. The server adds `project` itself whenever you apply `userstory`, so a
story always carries both, and `--label userstory --label project` is accepted.
`sprint`, `epic`, and `milestone` carry no such implication and must **not** be
combined with `project` or with each other; stories relate to those containers
through their `userstory.sprint`, `userstory.epic` and `userstory.milestone` fields
(`--field userstory.sprint=<id>`), never by sharing labels. The server keeps the
matching graph links in step; `links add` refuses to write them directly.

Other labels (e.g. `urgent`, `now`) are free-form, not prime — discover them with
`nestr labels list`. `now` is the label `plan add`/`plan remove` toggle.

## App fields (--field)

App labels carry namespaced fields (`sprint.term`, `sprint.capacity`,
`userstory.points`, `result.target_complete`, …). Set them on `nests create` and
`nests update` with the repeatable `--field KEY=VALUE` flag:

```sh
nestr nests create --title "Sprint 12" --label sprint --parent <circleId> \
  --field sprint.term='{"from":"2026-08-01","to":"2026-08-14"}' \
  --field sprint.capacity=20 --yes

nestr nests update <id> --field sprint.status=active --yes
```

KEY is `label.field` — `nestr nests get <id> --fields-meta` lists the codes for a
nest's labels. VALUE is typed by a JSON parse when it is valid JSON (`20` becomes a
number, `true` a boolean, `{…}` an object) and is sent as a plain string otherwise;
quote as JSON (`'"true"'`) to force a string. Unknown keys are passed through and
validated by the server.

## Purpose vs description

Two distinct fields on nests, circles, roles, and tension parts:

- **`--purpose`** — a single-line statement of *why* the thing exists. Central for
  circles and roles, optional-but-encouraged for a project. Purpose is **inherited**:
  a nest with no purpose of its own shows its parent's. Keep it to one line.
- **`--description`** — the *body*: the actual details/content.
- **`--due`** — a date, ISO format (e.g. `2026-07-01`). With `--start` it is the *end*
  of the span; without one it is the deadline (see [Dates & recurrence](#dates--recurrence)).

Never put body text in `--purpose` — that is the classic Nestr mistake.

## Dates & recurrence

A nest's schedule is `start` + `due` (+ `allDay`). `nests create` and `nests update` take:

- **`--start`** — when it begins: an ISO 8601 date-time (`2026-10-05T09:00:00+02:00`),
  or `yyyy-mm-dd` for a whole day. With `--due` it makes a span and `--due` is the end.
- **`--all-day`** — whole calendar days (`--all-day`, or `--all-day false` for instants).
  `yyyy-mm-dd` dates imply it and are all-day on UTC day edges, so they read as the same
  dates in every timezone.
- **`--clear-start`** (update only) — drop the start, leaving `--due` as a deadline.
- **`--clear-due`** (update only) — drop the due date (sends `due: null`). On a
  meeting, a `--start` in the same call re-derives the end from the meeting's
  duration, so the due is not cleared.

A date the server can't read, or a due before the start, comes back as a
`validation error:` with the server's reason, and nothing is written.

```bash
nestr nests create --title "Offsite" --parent <id> --start 2026-10-05 --due 2026-10-09 --all-day
nestr nests update <id> --start 2026-10-12T09:00:00+02:00 --due 2026-10-12T10:00:00+02:00
nestr nests update <id> --clear-start
nestr nests update <id> --clear-due
```

**Recurrence.** `nests recurrence` makes a dated nest repeat on an RFC 5545 RRULE.
Setting a rule creates **no** nests: occurrences past the next one are *virtual* (the
rule produces the instant, no nest exists), so `search` and `nests children` can't see
them. `nests recurrence occurrences` is the only way to list them. Each row has an
**INSTANT** (epoch milliseconds): that is how `skip` and `edit` address an occurrence,
so copy it as printed. A date is refused. Every command takes the id of the series or
of any occurrence of it.

```bash
nestr nests recurrence set <id> 'FREQ=WEEKLY;BYDAY=MO,WE;COUNT=10'   # quote it: ; is a shell separator
nestr nests recurrence occurrences <id>                  # next 10; --past for history, --cursor to page
nestr nests recurrence skip <id> <instant>               # skip one; the series carries on
nestr nests recurrence skip <id> <instant> --following   # delete it and every later one, keep history
nestr nests recurrence edit <id> <instant> --start 2026-10-14T09:00:00Z   # move just this one
nestr nests recurrence edit <id> <instant>               # no flags: materialize it as a real nest
nestr nests recurrence remove <id>                       # stop repeating, keep materialized nests
nestr nests recurrence delete-series <id>                # delete the series and all its history
```

- The rule expands from the nest's start (its due when it has none); set a date first.
- STATE in the listing is `virtual`, `nest` (materialized), `skipped`, plus `altered`
  (changed from the series) and `done`. A skipped instant stays listed.
- `edit` answers with the occurrence's own nest id. Use that id with every other `nests`
  command from then on. A refused edit can still have materialized the occurrence.
- The `seriesId` in `-o json` names the repeat itself. It is not a nest id, and
  `nests get` on it is a 404.
- `skip`, `skip --following` and `delete-series` ask for confirmation (`--yes` in
  scripts and agent mode) and print the nest to restore in the Nestr app to undo.
  Restoring brings back that nest only.
- Rights are the server's: skipping needs update on the series (plus delete on a
  materialized occurrence); `--following` and `delete-series` need delete on the series
  and on every occurrence they remove. A refusal is a `permission denied` with nothing
  written.
- To leave dates out, `skip` them. Don't bound the rule with COUNT and start a second
  series, because that leaves two nests with the same title.

## Proxy-assign

`--proxy-assign <roleOrCircleId>` on `nests create` and `nests update` does what the
assign dialog in the app does for work that sits inside a role or circle: the nest keeps
its parent, shows under the role as a mirror, and its assignee becomes the role's first
filler (nobody for a circle; an `--assignee` in the same call wins). `--clear-proxy-assign`
(update only), or naming the role or circle the nest already sits in, removes it. A nest
directly under a role or circle is moved there instead, and the server warns. Read it
back as `proxyAssign` with `nests get <id> -o json`.

```bash
nestr nests update <taskId> --proxy-assign <roleId>
nestr nests update <taskId> --clear-proxy-assign
nestr nests create --title "Draft the brief" --parent <projectId> --proxy-assign <roleId>
```

## Formatting

Text bodies — comment bodies, `--description`, and `--purpose` — render as **Markdown**
in the web app (headings, `**bold**`, `-`/`1.` lists, `> quotes`, `` `code` ``/fenced
blocks, `[links](url)`; bare URLs auto-link). Prefer Markdown over one flat line.

**Literal angle brackets are stripped.** The server HTML-sanitizes the body *before*
storing it, so tag-looking tokens — `<id>`, `Vec<String>`, `x < y` — lose the `<…>`.
Backticks don't protect them (the sanitizer runs before Markdown). Write `&lt;id&gt;`,
or rephrase (`{id}`, "x less than y").

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
`{host}/n/{parentId}/{nestId}#{tab}` (opens the nest in context, on the parent's tab
that lists it: `#projects`, `#roles`, `#tasks`, …). `nests get`/`create`/`update`
print this as a `url:` line in `-o text`; don't hand-build links from memory.

## Command map

| Group | What it does | Skill |
|---|---|---|
| `auth`, `profiles`, `me` | authenticate, switch profiles, identity | this reference |
| `search` | find nests across a workspace or subtree | nestr-basics |
| `nests` | read/create/update/delete/reorder nests + labels, dates, recurrence; `nests files` attachments | nestr-basics |
| `comments` | discuss on a nest | nestr-basics |
| `inbox`, `plan`, `work`, `activity` | capture, plan the day, open work, recent activity | nestr-basics |
| `notifications`, `labels`, `projects` | stay current, labels, projects | nestr-basics |
| `dm`, `queues` | direct messages, support queues | nestr-basics |
| `tensions` | governance: propose → consent → enact | nestr-governance |
| `workspaces`, `circles`, `roles`, `users`, `groups` | org structure & members; `workspaces docs` reference documents | nestr-org |
| `links`, `insights`, `hints`, `export` | graph links, metrics, hint counts, JSON dumps | nestr-insights |
| `webhooks` | workspace event subscriptions | nestr-webhooks |
| `agents` | create and run agents; reactive or autonomous mode per nest | nestr-agents |
| `connectors` | connector catalog, bindings to roles, connect links | nestr-agents |
