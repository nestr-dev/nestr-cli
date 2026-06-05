---
name: nestr-governance
description: Use when proposing or processing organizational change in Nestr via tensions — creating a tension, proposing role/circle/policy changes as parts, reviewing the diff, submitting for consent, and voting. The governance workflow for the `nestr tensions` commands.
---

# Nestr Governance (Tensions)

A **tension** is the gap between current reality and a desired state. In Nestr you
process tensions through governance: propose a change, the circle consents, it enacts.
See [`../shared/reference.md`](../shared/reference.md) for profiles, global flags,
and the full command map.

## Anatomy of a tension

```bash
nestr tensions create <circleId> --title "No one owns supplier relations" \
  --description "We missed two renewals" --feeling "anxious" --needs "reliability"
```
- **title** — the gap (current vs desired).
- **description** — observable facts.
- **feeling / needs** — optional; what it evokes and the need that is alive.

## Where does a tension belong?

- A tension one of **your roles** cares about → create it on that role.
- Something **another role** in your circle should own → create on the circle.
- A change to **how the circle is structured** → governance tension on the circle.

## Propose the change (parts)

A tension holds **parts** — the concrete proposals:

```bash
# Propose a NEW role/circle/policy
nestr tensions parts add <circleId> <tensionId> --title "Supplier Steward" --label role \
  --purpose "Reliable supplier relationships" --accountability "Owning renewals"

# Propose a CHANGE to an existing role/circle (give its id)
nestr tensions parts propose-update <circleId> <tensionId> --id <roleId> --purpose "Revised purpose"

# Propose REMOVING an existing item
nestr tensions parts propose-delete <circleId> <tensionId> --id <roleId>

# Manage a proposal's accountabilities/domains
nestr tensions parts children add <circleId> <tensionId> <partId> --title "X" --label accountability
```

Review exactly what will change before you submit:

```bash
nestr tensions parts list <circleId> <tensionId>
nestr tensions parts changes <circleId> <tensionId> <partId>   # variable: old → new
```

## The consent lifecycle

```bash
nestr tensions submit <circleId> <tensionId>     # draft → proposed (notifies the circle)
nestr tensions status <circleId> <tensionId>     # watch the vote tally
nestr tensions retract <circleId> <tensionId>    # pull it back to draft to edit
```

When a tension is **awaiting your consent**, you vote:

```bash
nestr tensions awaiting-consent                  # what needs your vote
nestr tensions vote <circleId> <tensionId> accept    # consent
nestr tensions vote <circleId> <tensionId> escalate  # object — one objection blocks consensus
```

`nestr tensions mine` lists tensions you created or are assigned to — check it at the
start of a session and after finishing work.

## Safety

`tensions delete` and `parts remove` ask for confirmation (pass `--yes` in scripts).
`--read-only` blocks every write.
