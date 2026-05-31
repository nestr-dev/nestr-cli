# nestr-cli

A fast, composable command-line interface for [Nestr](https://nestr.io), for terminal users and AI agents.

> Status: Phase 0 (foundation & auth). See `docs/superpowers/specs/` for the design and roadmap.

## Install (from source)

```bash
cargo install --path .
nestr --help
```

## Quick start

```bash
nestr profiles add            # configure a profile (OAuth or API key)
nestr me                      # verify authentication
```

## License

Apache-2.0. Derived from [coralogix/cx-cli](https://github.com/coralogix/cx-cli) (Apache-2.0); see `NOTICE`.
