---
name: nestr-insights
description: Use when relating nests with graph links, reading workspace insights/metrics, or exporting the work and governance trees as JSON from the terminal.
---

# Nestr Insights, Graph & Export

Three ways to see across a Nestr workspace: **graph links** connect nests,
**insights** report metrics over time, and **export** dumps whole trees as JSON.

First run `nestr profiles add` (OAuth or API key) — a profile pairs a **workspace**
with an **identity**. Global flags on every command: `-p/--profile`, `-o text|json`,
`--read-only` (block writes), and `--yes` (skip write confirmations; required for
agents). Full setup, env overrides, and the command map:
[shared/reference.md](https://github.com/nestr-dev/nestr-cli/blob/main/skills/shared/reference.md).

## Relate nests (graph links)

Links are **directed** and grouped by a free-text **relation** (e.g. `meeting`). A
link runs from a source nest to a target; the target lists it as incoming. Argument
order is the direction, so `links add A rel B` and `links add B rel A` are different
links, and `links remove` only removes the one in the order you give.

```bash
nestr links list <nestId>                       # all links of a nest, with direction
nestr links list <nestId> --relation meeting    # filter by relation
nestr links list <nestId> --direction in        # incoming only (in | out)
nestr links add <dependentId> depends_on <predecessorId>  # dependent waits for predecessor
nestr links remove <dependentId> depends_on <predecessorId> # remove that link
```

Some links are written by the server as the result of another change, and `links add`
/ `links remove` refuse them with the setting to change instead. A link field on the
source nest (a tension's `meeting`, a story's sprint, epic or milestone) is set with
`--field`, and a proxy-assignment with `--proxy-assign`:

```bash
nestr nests update <tensionId> --field tension.meeting=<meetingId>  # put it on the agenda
nestr nests update <storyId> --field userstory.sprint=<sprintId>
nestr nests update <taskId> --proxy-assign <roleId>
```

`recurrence` links follow a series, so change the series with `nests recurrence`;
`templateCopy` links follow a copy's `data.copiedFrom`.

A `depends_on` link that would create a cycle is refused. For other relations, a link
that already runs the other way is refused rather than doubled: remove it first. If
`links remove` finds the link only in the other direction, the error prints the
command that removes it.

## Read metrics (insights)

```bash
nestr insights list                      # workspace metrics (roles, tensions, …)
nestr insights get <metricId>            # one metric's current value
nestr insights history <metricId> --from 2026-01-01 --to 2026-06-01
```

Insights is **BETA**. All insights commands need the **Insights app** enabled in the
workspace (else `403`); filtering by `--user`/`--circle` additionally needs a **Pro**
plan (else `402`).

## Count health signals (hints)

A hint on one nest says "this one has this problem"; `nestr hints` says how many
nests under a circle (or the whole workspace) have it, in one call:

```bash
nestr hints                                   # the active workspace
nestr hints <circleId> --min-severity warning # what needs attention in a circle
nestr hints <circleId> --type unassigned_role --sample-size 0   # one number
```

Each row is a hint type, its severity, how many nests carry it, and a few examples.
Don't add the counts up: one nest can carry several hints, so ask for a single
`--type` when you want a number that sums. Types listed under **not computed** can't
be counted in one query: they are unknown, not zero. To open every nest behind a
count, read one with `nestr nests get <id> --hints=summary` and run the search it
suggests.

## Export (always JSON)

```bash
nestr export work > work.json               # the workspace work view
nestr export governance > governance.json   # the full governance tree
nestr export work | jq '.'                  # or pipe straight to jq
```

`export` always emits JSON regardless of `-o`.

## Safety

Everything here (including `hints`) is read-only except `links add`/`links remove`, which are blocked
by `--read-only`. Add `-o json` to `links`/`insights` for raw structured output.
