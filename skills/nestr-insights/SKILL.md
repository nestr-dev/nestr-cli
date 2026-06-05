---
name: nestr-insights
description: Use when relating nests with graph links, reading workspace insights/metrics, or exporting the work and governance trees as JSON from the terminal.
---

# Nestr Insights, Graph & Export

Three ways to see across a Nestr workspace: **graph links** connect nests,
**insights** report metrics over time, and **export** dumps whole trees as JSON.
See [`../shared/reference.md`](../shared/reference.md) for profiles and global flags.

## Relate nests (graph links)

Links are bidirectional and grouped by a free-text **relation** (e.g. `meeting`):

```bash
nestr links list <nestId>                       # all links of a nest
nestr links list <nestId> --relation meeting    # filter by relation
nestr links list <nestId> --direction in        # incoming only (in | out)
nestr links add <nestId> meeting <targetId>     # link two nests via "meeting"
nestr links remove <nestId> meeting <targetId>  # unlink
```

## Read metrics (insights)

```bash
nestr insights list                      # workspace metrics (roles, tensions, …)
nestr insights get <metricId>            # one metric's current value
nestr insights history <metricId> --from 2026-01-01 --to 2026-06-01
```

Filtering insights by `--user`/`--circle` needs a **Pro** plan, and the **Insights
app** must be enabled in the workspace — otherwise the API returns 403.

## Export (always JSON)

```bash
nestr export work > work.json               # the workspace work view
nestr export governance > governance.json   # the full governance tree
nestr export work | jq '.'                  # or pipe straight to jq
```

`export` always emits JSON regardless of `-o`.

## Safety

Everything here is read-only except `links add`/`links remove`, which are blocked
by `--read-only`. Add `-o json` to `links`/`insights` for raw structured output.
