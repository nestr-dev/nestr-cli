---
name: nestr-basics
description: Use when working with Nestr from the terminal — searching and reading nests, creating projects and subtasks, capturing to the inbox, planning the day, posting comments and updates, direct messages and support queues, and checking notifications and your open work. The everyday `nestr` CLI loop for an individual contributor.
---

# Nestr Basics

`nestr` is a fast, composable CLI for Nestr. Everything in Nestr is a **Nest**, and
what a nest *is* is set by a single **prime label** (`project`, `goal`, `role`, …) —
a nest with no prime label is a plain todo, labelled `task`. Pick a profile once (`nestr profiles add`),
then work the loop below. Add `-o json` to any command for raw JSON to pipe to `jq`.
Full setup, global flags, and the command map:
[shared/reference.md](https://github.com/nestr-dev/nestr-cli/blob/main/skills/shared/reference.md).

## Find things

```bash
nestr search "quarterly review"          # search the active workspace
nestr search "bug" --in <nestId>         # search within a nest subtree
nestr search "label:project completed:false" --strict   # fail on a mistyped filter
nestr nests get <id>                     # read one nest (or several: id1,id2)
nestr nests get <id> --hints             # plus its health signals (or --hints=summary)
nestr nests children <id>                # list a nest's children
nestr nests children <id> --search "label:role"   # just the children you want
```

**Counting? Use `--strict`.** Without it an unrecognised operator, label or field
filter is dropped silently and you get a real, larger answer. `--strict` turns that
into an error that names the filter.

`--hints` adds contextual signals (unfilled roles, stale work, waiting projects, …),
each with the search that finds every other nest with the same problem. On
`nests children` it defaults to `summary`; filter with `--hint-types` and
`--min-severity warning`. To count them across a whole circle, see `nestr hints` in
nestr-insights.

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
nestr inbox move <id> before <otherId>   # move one inbox item (before | after)
nestr plan add <id>                      # put a nest on today's plan
nestr plan today                         # see today's plan
```

## Create projects, todos & subtasks

What a nest **is** is set by one **prime label** (`project`, `goal`, `result`,
`checklist`, `meeting`, `metric`, `feedback`, `circle`, `role`, `anchor-circle`,
`tension`, `userstory`, `sprint`, `epic`, `milestone`). No prime label = a plain
todo, which the server labels `task` automatically; pass `--label task` to be
explicit, and search todos with `label:task`. There's no `project create` — `nests
create --label <prime>` makes each kind, and `--parent` nests it as a subtask.

```bash
nestr nests create --title "Fix the login bug" --parent <projectId> --label task --assignee me   # a todo, assigned to you
nestr nests create --title "Write the spec" --parent <id> --label project \
  --description "Scope, milestones, open questions" --due 2026-07-01 --assignee me   # a project
nestr nests create --title "Pre-launch checklist" --parent <projectId> --label checklist
```

- `--label project` (or any prime) is what makes it that kind — **omit it and you get a
  plain `task`-labelled todo, not a project.** A nest can carry only one prime label,
  with one exception: `userstory` implies `project`, so a story legitimately carries
  both (the server adds `project` for you). `sprint`/`epic`/`milestone` never pair with
  `project` — link stories to them with `nestr links add` instead.
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

## Dates & repeats

```bash
nestr nests update <id> --start 2026-10-05 --due 2026-10-09 --all-day   # a whole-day span
nestr nests recurrence set <id> 'FREQ=WEEKLY;BYDAY=MO'                    # repeat weekly (needs a date)
nestr nests recurrence occurrences <id>                                  # list them, with each INSTANT
nestr nests recurrence skip <id> <instant>                               # skip one week
nestr nests recurrence edit <id> <instant> --title "Sync (with guests)"  # change one occurrence
```

Occurrences past the next one are **virtual**: no nest exists until one is touched, so
`search` won't find them. Address an occurrence by the INSTANT that `occurrences` prints,
not by a date. `--start` with `--due` makes `--due` the end of the span. Details, and
`remove` vs `delete-series`, are in the shared reference under "Dates & recurrence".

## Files on a nest

Attachments hang off a nest or a comment (pass the comment id as the nest id).

```bash
nestr nests files list <nestId>                       # id, name, type, size
nestr nests files read <nestId> <fileId>              # prints a text/JSON file
nestr nests files read <nestId> <fileId> --save ./spec.pdf   # any file, to disk
nestr nests files upload <nestId> ./notes.md          # type guessed from the extension
nestr nests files upload <nestId> ./export --name data.csv --content-type text/csv
nestr nests files delete <nestId> <fileId> --yes      # permanent
```

`read` only prints text types; anything else needs `--save` (it won't overwrite an
existing file without `--force`). `-o json` returns the file with its bytes in
`dataBase64`. Images pasted into a nest's text are not in `list`, but their ids (from
the `/file/download?id=…` reference in the content) work with `read`. Uploads over the
server's limit (10 MB by default) are rejected.

## Discuss & post updates

Record progress as **comments** on the project or todo — don't rewrite the nest's
`--description` to log what happened.

```bash
nestr comments add <nestId> "Spec signed off; implementation starts Monday"   # post an update
nestr comments add <nestId> "## Status
- API **done**, deploying Friday
- blocked on auth review — see [the PR](https://example.com/pr/1)"            # markdown renders
nestr comments list <nestId>                                                  # read the thread
nestr comments list <nestId> --unread                                         # only what you have not read
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
nestr dm list --unread                   # conversations with something new
nestr queues threads support --unread    # support threads that have moved
```

## Direct messages & support queues

A direct-message **thread** is addressed by its id everywhere; there is no container to
look up first. These commands need an OAuth login (`nestr auth login`), not an API key.

```bash
nestr dm list                            # open threads, most recently posted first
nestr dm list --with <user> --include-closed   # one person's threads, closed ones too
nestr dm start <user> --title "Budget"   # new thread (user id, username or email)
nestr dm show <threadId> --unread        # the thread plus the posts you have not read
nestr dm posts <threadId>                # every post, oldest first
nestr dm post <threadId> "On it, back by Friday"
nestr dm mark-read <postId>              # read up to this post (works for any post)
nestr dm close <threadId>                # done: drops out of `dm list`, still readable
nestr dm reopen <threadId>
nestr dm update <threadId> --title "Q4 budget"
nestr dm update <threadId> --users <me>,<them>,<colleague>   # replaces the participant list
```

`dm update --users` sets the **whole** participant list, so read it with `dm show` first and
send it back with someone added or removed; send it without yourself to leave. It asks for
confirmation (`--yes` in scripts and agents).

A support queue is a label on DM threads. If you monitor the queue you see every thread in
it; otherwise only the ones you raised, which is how you find your own open tickets. Read
a queue thread with `nestr dm show <threadId>`.

```bash
nestr queues list                        # queues you monitor or have a thread in
nestr queues threads support             # threads in the `support` queue
nestr queues threads support --read      # only the ones you have already read
nestr work                               # your open projects + todos
nestr projects list                      # every project in the workspace
nestr activity                           # what you've done lately, across workspaces
nestr activity --user <userId>           # a colleague's (or an agent's) activity
```

`activity` needs a user-scoped (OAuth) login; a workspace API key has no user to
report on. An OAuth login granted for a workspace only gets a "single user scoped
access" error: run `nestr auth login` again and grant access to your own user account.

## Safety

Destructive commands (`nests delete`, `nests recurrence skip`/`delete-series`, `comments delete`, `nests files delete`) ask for confirmation;
pass `--yes` to skip it (required in agent/non-interactive contexts). Use
`--read-only` to hard-block every write.
