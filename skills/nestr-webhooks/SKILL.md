---
name: nestr-webhooks
description: Use when managing Nestr workspace webhooks from the terminal — listing, inspecting, creating, and deleting event subscriptions.
---

# Nestr Webhooks

A **webhook** subscribes an external URL to workspace events — when a nest or
comment is created, updated, or deleted, Nestr POSTs to your URL. Managing webhooks
is **admin-only** — a non-admin token gets a 403.

First run `nestr profiles add` (OAuth or API key) — a profile pairs a **workspace**
with an **identity**. Global flags on every command: `-p/--profile`, `-o text|json`,
`--read-only` (block writes), and `--yes` (skip write confirmations; required for
agents). Full setup, env overrides, and the command map:
[shared/reference.md](https://github.com/nestr-dev/nestr-cli/blob/main/skills/shared/reference.md).

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

- `--url` — the endpoint Nestr POSTs to (**required**).
- `--type` — `nest` or `comment`.
- `--event` — `create`, `update`, or `delete`.
- `--label` — only fire for items carrying this label (optional).
- `--ancestor` — only fire for events under this nest (optional).

Creating a duplicate subscription (same url + type + event) returns a 400.

## Delete (admin)

```bash
nestr webhooks delete <id>               # asks to confirm; pass --yes to skip
```

## Safety

`webhooks create`/`delete` are blocked by `--read-only`; `delete` asks for
confirmation (pass `--yes` in scripts/agents). Add `-o json` for raw output.
