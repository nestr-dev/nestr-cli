---
name: nestr-insights
description: Use when relating nests with graph links, reading workspace insights/metrics for a period or comparing circles, reading or backfilling a nest's dataseries points (metric, goal or burndown history), or exporting the work and governance trees as JSON from the terminal.
---

# Nestr Insights, Graph & Export

Four ways to see across a Nestr workspace: **graph links** connect nests,
**insights** report metrics over time, **dataseries** hold a nest's dated points
(a metric's or goal's history), and **export** dumps whole trees as JSON.

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
nestr insights list                                  # workspace metrics for the last month
nestr insights list --from 2026-10-01 --to 2026-11-01  # metrics for a chosen period
nestr insights list --nest <circleId>                # one circle (Pro)
nestr insights list --user me                        # your own metrics (Pro, OAuth login)
nestr insights get <metricId> --to 2026-11-01        # one metric for a period
nestr insights history <metricId> --from 2026-01-01 --to 2026-06-01
nestr insights history <metricId> --nest <circleId>  # one circle's series (Pro)
nestr insights compare <metricId> --parent <circleId>  # its child circles side by side (Pro)
```

A value is only meaningful with its period, so text output starts with the exact
dates the server analysed (`period`) and compared against (`compared`). Without
`--from`, the period is the month before `--to` (default: now). The compare period
has the same length and ends where the period starts. Under the table, each kind of
metric present gets one line from the server saying how it reads: a
`point_in_time` value is the value at the period's end, compared with the last
value before it began; a `cumulative` value is a total within each period. Under
`-o json`, stdout stays the bare array and the periods go to stderr as one
`{"meta": …}` line.

`--nest` and `--user` can't be combined: personal metrics cover the whole
workspace. `--user me` needs a login that acts for one person (an OAuth profile),
not a workspace API key. `--end-date` still works as a hidden alias of `--to`, and
`--circle` as an alias of `--nest`.

`compare` lists each child circle's value, compare value, change and direction, and
the metric's display format (such as `percentage`).
The parent row is marked `(parent)`: its value includes its children, so don't rank
it against them.

Insights is **BETA**. All insights commands need the **Insights app** enabled in the
workspace (else `403`). One circle, one person, and `compare` need a **Pro** plan:
below Pro the server answers `402` with a message saying what Pro adds, which the
CLI prints as is. The API returns no per-metric chart link. To link a chart in the
web app, the pattern is `/n/<circleId>?i=1&show_chart=<metricType>`.

`insights history` covers Nestr's usage metrics only. The history of a metric nest,
a goal or a burndown is a dataseries on that nest: read it with `nests series`.

## Dataseries points (metric, goal and burndown history)

A dataseries field holds dated points, and the field it tracks shows the newest
point at or before now. Setting that field with `nests update` records one point,
now. To load past values or correct history, write the points:

```bash
nestr nests series list <nestId>                       # its series: key, kind, tracked field, current point
nestr nests series points <nestId> <key> --from 2026-01-01 --order desc
nestr nests series add <nestId> <key> 2026-01-05=12 2026-01-12=14   # backfill DATE=VALUE points
nestr nests series add <nestId> <key> --file points.json            # or a JSON array of {date, value}
nestr nests series edit <nestId> <key> <pointId> --value 13
nestr nests series delete <nestId> <key> <pointId>
```

- Address a series by the `key` that `list` prints (a field code, or the field id
  when two fields share a code). On a multi-series field, also pass `--series <id>`.
- `add` takes any number of points and sends them 500 at a time. Each point is
  kept exactly, and several can share a day. A refused point doesn't stop the
  rest: each one is listed with the server's reason, and the command exits non-zero.
  A future-dated point doesn't become the current value.
- An edited point is kept as you set it: a later write to the tracked field no
  longer replaces it.
- A series `list` marks **hidden** isn't in use on this nest. You can read it, but
  writes to it are refused.
- `points` returns up to 500 points (`--limit`, max 5000). When more remain, it
  says so: narrow `--from`/`--to` to read the rest.
- `add`, `edit` and `delete` need update rights on the nest and are blocked by
  `--read-only`. `delete` asks first, or needs `--yes` from an agent.

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

Everything here (including `hints`) is read-only except `links add`/`links remove` and
`nests series add`/`edit`/`delete`, which are blocked by `--read-only`. Add `-o json` to `links`/`insights` for raw structured output.
