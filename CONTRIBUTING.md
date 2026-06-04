# Contributing to Stardex

Thanks for helping build **Stardex** — the open-source indexer & data API for Stellar / Soroban. 🌌

Stardex is listed on [GrantFox](https://www.grantfox.xyz/): claim an issue, open a PR, and earn USDC on merge.

## Quick start

```bash
# Rust "engine" wing (core watcher + decoders + CLI)
cd ingestor
cargo build
cargo test
cargo run -p stardex-cli -- decoders list

# TypeScript "web" wing (types, sdk, api, frontend)
pnpm install
pnpm -r build
pnpm -r typecheck
```

## Where to start

- 🌱 **First time?** Pick a [`good first issue`](./docs/BACKLOG.md) — most are a single decoder, endpoint, component, or doc page.
- 🧩 **Highest leverage:** write a **decoder** for a contract you already use (tutorial: backlog issue #30). Adding a decoder never touches the core engine.
- See [`docs/BACKLOG.md`](./docs/BACKLOG.md) for the full, labeled backlog.

## Ground rules

1. **One issue per PR.** Keep changes focused and easy to review.
2. **Comment on the issue before starting** so it can be assigned to you (this is how GrantFox tracks contributions).
3. **Add tests** for decoders and API logic, with sample fixtures.
4. **Format & lint** before pushing: `cargo fmt` + `cargo clippy` for Rust, `pnpm lint` for TypeScript.
5. **Link your PR to the issue** it resolves (`Closes #NN`).

## Project layout

See the **Planned repo layout** section of the [README](./README.md) for what each folder does.

## Code of conduct

Be respectful and constructive. We're here to grow the Stellar ecosystem together.
