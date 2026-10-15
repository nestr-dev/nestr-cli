---
name: nestr-agents
description: Use when setting up or operating Nestr agents and connectors from the terminal — creating an agent, running it on a nest, setting it reactive or autonomous on a cadence, registering connectors, binding them to a role, and handing a person the link to connect an account.
---

# Nestr Agents & Connectors

An **agent** is a bot user in the workspace. It does work when it fills a role or
is assigned to a task. A **connector** is a catalog entry for an outside tool (an
MCP server, an API or a CLI). A **connection** binds a connector to an owner, usually
a role's domain, and a person connects an account to it through a link.

First run `nestr profiles add` (OAuth or API key) — a profile pairs a **workspace**
with an **identity**. Global flags on every command: `-p/--profile`, `-o text|json`,
`--read-only` (block writes), and `--yes` (skip write confirmations; required for
agents). Full setup, env overrides, and the command map:
[shared/reference.md](https://github.com/nestr-dev/nestr-cli/blob/main/skills/shared/reference.md).

**Who can run what.** Commands marked *admin* need a workspace admin; they also work
with a workspace API key. Commands marked *user login* need an OAuth or user login,
because the server records who asked: a workspace API key gets a 403 there.

## Agents

```bash
nestr agents list                                   # the workspace's agents
nestr agents get <agentId>
nestr agents create --name Collab                   # admin
nestr agents create --name Helper --assistant \
  --description "Careful with anything that spends money"
nestr agents connectors <agentId>                   # what it can use, and why not
```

- `--name` is the agent's own name, not the name of the work. The work belongs to
  the role the agent fills.
- `--description` is what the agent is *like*. Its instructions belong in a skill
  nest under the role.
- `--assistant` makes an agent that can never fill a role, so it always acts with
  the authority of the person it helps. Put it on the task beside that person.
- `--config '<json>'` sets runtime wiring (`runtimeCallbackUrl`, `tokenTtlSeconds`).
  Leave it out for an agent that runs on Nestr's own runtime.

## Run an agent now (user login)

```bash
nestr agents run <agentId> --nest <nestId>
nestr agents run <agentId> --nest <nestId> --message "Draft the Q3 summary"
```

You need assign rights on the nest, and the agent must fill or be assigned to it.
The output gives a watch link where the agent reports back. Pass it to whoever asked.

## Reactive or autonomous

The mode is set per nest, never on the agent as a whole. Items under a nest inherit
its setting unless they set their own.

```bash
nestr agents mode get <agentId> --nest <nestId>     # mode, cadence, and where it comes from
nestr agents mode set <agentId> --nest <nestId> --mode reactive
nestr agents mode set <agentId> --nest <nestId> --mode autonomous --recheck continuous
nestr agents mode set <agentId> --nest <nestId> --mode autonomous \
  --recheck "FREQ=WEEKLY;BYDAY=MO,WE,FR" --start 2026-10-19T09:00:00 --tz Europe/Amsterdam
nestr agents mode clear <agentId> --nest <nestId>   # inherit again
```

`mode set` and `mode clear` need a user login.

- `--recheck` is required with `autonomous`. Use `continuous` to work the item until
  it is done or blocked, or an RRULE (`hourly`, `daily` and `weekly` are shorthand).
  Nothing runs more often than hourly.
- `--start` sets when the schedule starts and the time of day it runs. A time with no
  offset is read in `--tz`, which defaults to UTC.
- A cadence re-checks **one item** until it is done. For a job that should happen
  again and again, like a weekly report, use a repeating task assigned to the agent.

## Connectors (admin)

```bash
nestr connectors list                               # the catalog
nestr connectors templates                          # one-click vendors this deployment offers
nestr connectors register --template <templateId>
nestr connectors register --type mcp --name "Internal tools" \
  --config '{"url":"https://mcp.example.com"}' --auth-strategy oauth2
nestr connectors update <connectorId> --disable     # pause without removing bindings
nestr connectors remove <connectorId>               # asks to confirm; bindings stop resolving
```

**Use a template for a known vendor.** A hand-registered vendor needs its endpoint
guessed, and a wrong guess authorises fine and then fails every call. Without a
template, `--type` and `--name` are required. `mcp` and `api` need a `url` in
`--config`, and `cli` needs a `command`.

`--config`, `--capabilities` and `--exposure` take JSON objects. They hold no
secrets: the server refuses a header that looks like a credential.

## Give access (admin)

```bash
nestr connectors bind <connectorId> --owner-type role --owner <roleId>
nestr connectors connections list                   # who has access to what
nestr connectors connections list --include-disabled
```

**Bind to the role**, not to whoever fills it. The access then belongs to the work and
survives a change of filler. The server finds or creates the connector's domain under
the role. The other owner types:

- `role-domain`: an existing domain nest, when you already have its id.
- `workspace`: everyone in the workspace.
- `user` or `agent`: one person's or one agent's own account, such as their own
  mailbox. This needs the connector to allow personal owners.

## Connect an account

```bash
nestr connectors connections link <connectionId>
```

This prints a link. **Give it to a person to open.** They sign in or enter the key
there, so the secret never passes through the CLI or an agent. Don't fetch the link
yourself, and don't ask anyone for a secret. Run `connections list` afterwards to
check that it worked.

## Remove access

```bash
nestr connectors connections revoke <connectionId>  # your own credential; the binding stays
nestr connectors connections remove <connectionId>  # admin: the binding and every credential on it
```

## Safety

Every write (`create`, `run`, `mode set`/`clear`, `register`, `update`, `remove`,
`bind`, `revoke`) is blocked by `--read-only`. `remove` and `revoke` ask for
confirmation (pass `--yes` in scripts/agents). `connections link` changes nothing and
works under `--read-only`. No command takes or prints a credential. Add `-o json` for
raw output. Server hints, such as "use a template", go to stderr.
