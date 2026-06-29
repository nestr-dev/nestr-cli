---
name: nestr-basics
description: Use when working with Nestr from the terminal — searching and reading nests, creating projects and subtasks, capturing to the inbox, planning the day, posting comments and updates, and checking notifications and your open work. The everyday `nestr` CLI loop for an individual contributor.
---

# Nestr Basics

`nestr` is a fast, composable CLI for Nestr. Everything in Nestr is a **Nest**, and
what a nest *is* is set by a single **prime label** (`project`, `goal`, `role`, …) —
a nest with no prime label is a plain todo. Pick a profile once (`nestr profiles add`),
then work the loop below. Add `-o json` to any command for raw JSON to pipe to `jq`.
Full setup, global flags, and the command map:
[shared/reference.md](https://github.com/nestr-dev/nestr-cli/blob/main/skills/shared/reference.md).

## Find things

```bash
nestr search "quarterly review"          # search the active workspace
nestr search "bug" --in <nestId>         # search within a nest subtree
nestr nests get <id>                     # read one nest (or several: id1,id2)
nestr nests children <id>                # list a nest's children
```

`search` is **workspace-scoped** — it only looks in the active workspace (or the
`--in` subtree); there is no cross-workspace search. So if a search comes back empty,
don't conclude the nest doesn't exist — it may live in a **different** workspace. List
the reachable ones and offer to look there, but **ask the user before switching**:

```bash
nestr workspaces list                    # see which workspaces this identity can reach
nestr workspaces use <id>                # switch the active workspace, then search again
```

(Or scope a single search to another workspace with the global `-w <id>` flag.)

## Capture & plan

```bash
nestr inbox create "Call the supplier"   # capture to your inbox
nestr inbox list                         # see open inbox items
nestr plan add <id>                      # put a nest on today's plan
nestr plan today                         # see today's plan
```

## Create projects, todos & subtasks

What a nest **is** is set by one **prime label** (`project`, `goal`, `result`,
`checklist`, `meeting`, `metric`, `feedback`, `circle`, `role`, `anchor-circle`,
`tension`). No prime label = a plain todo. There's no `project create` — `nests
create --label <prime>` makes each kind, and `--parent` nests it as a subtask.

```bash
nestr nests create --title "Fix the login bug" --parent <projectId> --assignee me   # a plain todo, assigned to you
nestr nests create --title "Write the spec" --parent <id> --label project \
  --description "Scope, milestones, open questions" --due 2026-07-01 --assignee me   # a project
nestr nests create --title "Pre-launch checklist" --parent <projectId> --label checklist
```

- `--label project` (or any prime) is what makes it that kind — **omit it and you get a
  plain todo, not a project.** A nest can carry only one prime label.
- `--assignee` sets who does the work (the nest's `users`). **A project or task created
  with no `--assignee` is unassigned — it shows under nobody's work.** Pass `me` for
  yourself, or a user id from `nestr users list` (repeatable for several people).
- `--purpose` is an optional one-line *why* (a project inherits its parent's if unset);
  `--description` is the body. Never put body text in `--purpose`.
- `--parent <id>` makes the new nest a child/subtask — this is how you build the tree.

```bash
nestr nests update <id> --completed true                # tick it off
nestr nests update <id> --due 2026-07-01                # edit a field
nestr nests update <id> --assignee <userId>             # (re)assign — `me` for yourself
nestr nests label add <id> <labelId>                    # add one label (ids/codes via `nestr labels list`)
nestr nests label remove <id> <labelId>                 # remove one label
```

`nests update --label …` **replaces** the whole label set (re-list any you want to keep);
`nests label add/remove` toggle a single label without touching the rest. `--assignee`
likewise replaces the whole assigned set.

## Discuss & post updates

Record progress as **comments** on the project or todo — don't rewrite the nest's
`--description` to log what happened.

```bash
nestr comments add <nestId> "Spec signed off; implementation starts Monday"   # post an update
nestr comments add <nestId> "## Status
- API **done**, deploying Friday
- blocked on auth review — see [the PR](https://example.com/pr/1)"            # markdown renders
nestr comments list <nestId>                                                  # read the thread
```

Comment bodies (and `--description` / `--purpose`) render as **Markdown** in the web app
— headings, `**bold**`, `-`/`1.` lists, `> quotes`, `` `code` `` / fenced blocks, and
`[links](url)` all work (bare URLs auto-link). Write multi-point updates in Markdown
rather than one flat line.

**Pitfall — literal angle brackets are stripped.** The server runs the body through an
HTML sanitizer *before* storing it, so anything that looks like a tag — `<id>`,
`Vec<String>`, `x < y` — loses the `<…>` part. Backticks do **not** protect it (the
sanitizer runs before Markdown). Write `&lt;id&gt;` for a literal `<id>`, or rephrase
(`{id}`, "x less than y").

## Stay current

```bash
nestr notifications list                 # unread notifications
nestr notifications read                 # mark all read
nestr work                               # your open projects + todos
nestr projects list                      # every project in the workspace
```

## Safety

Destructive commands (`nests delete`, `comments delete`) ask for confirmation;
pass `--yes` to skip it (required in agent/non-interactive contexts). Use
`--read-only` to hard-block every write.
