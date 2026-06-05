---
name: nestr-basics
description: Use when working with Nestr from the terminal — searching, reading and editing nests, capturing to the inbox, planning the day, and commenting. The everyday `nestr` CLI loop for an individual contributor.
---

# Nestr Basics

`nestr` is a fast, composable CLI for Nestr. Everything in Nestr is a **Nest**
(projects, todos, comments, inbox items — all nests distinguished by labels).
Pick a profile once (`nestr profiles add`), then work the loop below. Add
`-o json` to any command to get raw JSON for piping to `jq`. See
[`../shared/reference.md`](../shared/reference.md) for profiles, global flags, and
the full command map.

## Find things

```bash
nestr search "quarterly review"          # search the whole workspace
nestr search "bug" --in <nestId>         # search within a nest subtree
nestr nests get <id>                     # read one nest (or several: id1,id2)
nestr nests children <id>                # list a nest's children
```

## Capture & plan

```bash
nestr inbox create "Call the supplier"   # capture to your inbox
nestr inbox list                         # see open inbox items
nestr plan add <id>                      # put a nest on today's plan
nestr plan today                         # see today's plan
```

## Create & update work

```bash
nestr nests create --title "Write spec" --parent <id> --label project
nestr nests update <id> --completed true
nestr nests label add <id> urgent
```

## Discuss

```bash
nestr comments add <nestId> "Looks good — shipping Friday"
nestr comments list <nestId>
```

## Stay current

```bash
nestr notifications list                 # unread notifications
nestr notifications read                 # mark all read
nestr work                               # your open projects + todos
```

## Safety

Destructive commands (`nests delete`, `comments delete`) ask for confirmation;
pass `--yes` to skip it (required in agent/non-interactive contexts). Use
`--read-only` to hard-block every write.
