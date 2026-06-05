---
name: nestr-webhooks
description: Use when managing Nestr workspace webhooks from the terminal — listing, inspecting, creating, and deleting event subscriptions.
---

# Nestr Webhooks

A **webhook** subscribes an external URL to workspace events — when a nest or
comment is created, updated, or deleted, Nestr POSTs to your URL. Managing webhooks
is **admin-only**. See [`../shared/reference.md`](../shared/reference.md) for
profiles and global flags.

## List & inspect

```bash
nestr webhooks list                      # the workspace's webhooks
nestr webhooks get <id>                  # one webhook
```

## Create (admin)

```bash
nestr webhooks create --url https://example.com/hook --type nest --event create
nestr webhooks create --url https://example.com/hook --type comment --event update \
  --label urgent --ancestor <nestId>
```

- `--type` — `nest` or `comment`.
- `--event` — `create`, `update`, or `delete`.
- `--label` — only fire for items carrying this label (optional).
- `--ancestor` — only fire for events under this nest (optional).

## Delete (admin)

```bash
nestr webhooks delete <id>               # asks to confirm; pass --yes to skip
```

## Safety

`webhooks create`/`delete` are blocked by `--read-only`; `delete` asks for
confirmation (pass `--yes` in scripts/agents). Add `-o json` for raw output.
