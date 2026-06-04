# 🌌 Stardex

> **An open-source indexer & data API for Stellar / Soroban — record every contract event, query any moment in history.**

[![Status](https://img.shields.io/badge/status-early%20development-orange)](#roadmap)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](./LICENSE)
[![Built on Stellar](https://img.shields.io/badge/built%20on-Stellar%20%2F%20Soroban-7B5BFF)](https://stellar.org)

---

## The problem

Stellar's RPC is built for *recent* data. It keeps only a short window of history and then prunes it — like a security camera that records over its own footage. That makes one of the most common questions in any app **surprisingly hard to answer**:

- "Show me every payment this user has made over the last 6 months."
- "What was this contract's volume, day by day?"
- "List all the streams / swaps / mints this contract has ever emitted."

Today, **every Soroban dApp team rebuilds the same indexing plumbing from scratch** — a service that watches the chain, copies events into a normal database, and exposes them for querying. It's duplicated effort across the entire ecosystem, and there is no canonical open-source tool that does it.

## What Stardex is

Stardex is that plumbing, **built once, as a tool everyone can use.** Point it at a contract and it:

1. **Watches** the Stellar/Soroban chain in real time.
2. **Decodes** each contract's events into clean, typed records.
3. **Stores** them durably in Postgres (so old history is never lost and is fast to search).
4. **Serves** them through a clean **GraphQL + REST API**, a typed **TypeScript SDK**, and a **web dashboard**.

If the blockchain is a live broadcast you can only watch *right now*, **Stardex is the DVR + search bar** — record everything, rewind to any moment, query instantly.

## Why it matters

- 🔌 **Every dApp needs it.** Historical event/state queries are a universal requirement. Stardex removes the need to build indexing yourself.
- 🛠️ **A tool contributors actually use.** Backend, frontend, and protocol devs all benefit on their own projects — which makes it a natural magnet for open-source contribution.
- 🧩 **Extensible by design.** Support for a new contract = writing a small **decoder**, never touching the core engine.
- 🪶 **No exotic dependencies.** Plain Rust, Postgres, TypeScript, and React. No ZK, no special hardware.

---

## Architecture

Stardex follows a clean **engine + decoders** split (the core stays neutral; new behavior is added as decoders):

```
            ┌──────────────────────────────────────────────┐
            │                  Stardex                      │
            │                                               │
 Stellar    │   ┌──────────┐   ┌──────────┐   ┌─────────┐   │
 RPC  ─────▶│   │ Ingestor │──▶│ Decoders │──▶│ Postgres│   │
 (events)   │   │  (core)  │   │ (per-    │   │  store  │   │
            │   └──────────┘   │ contract)│   └────┬────┘   │
            │      cursors,    └──────────┘        │        │
            │      reorgs,                          ▼        │
            │      backfill              ┌─────────────────┐ │
            │                            │  API (GraphQL   │ │
            │                            │   + REST)       │ │
            │                            └───┬──────┬──────┘ │
            └────────────────────────────────┼──────┼───────┘
                                             ▼      ▼
                                         TS SDK   Dashboard
```

- **Ingestor (core, Rust)** — streams events from RPC, tracks cursors/checkpoints, handles reorgs and backfill. Opinion-free about *what* the events mean.
- **Decoders (Rust)** — pluggable, per-contract decoders that turn raw event XDR into typed rows (e.g. token transfers, Soroswap swaps, payment streams). **This is where most contributions happen.**
- **Store (Postgres)** — durable schema + migrations + retention policies.
- **API (TypeScript)** — GraphQL + REST over the indexed data, with pagination and filtering.
- **SDK (`@stardex/sdk`) + types (`@stardex/types`)** — typed client so apps consume the API in a few lines.
- **Dashboard (React)** — browse indexed contracts, inspect events, view charts.
- **CLI (`stardex`)** — `stardex index <contract>`, manage decoders, run backfills.

## Tech stack

| Layer | Stack |
|-------|-------|
| Ingestor & decoders | Rust, Stellar RPC, Soroban XDR |
| Storage | PostgreSQL + SQL migrations |
| API | TypeScript (GraphQL + REST) |
| SDK / types | TypeScript (`pnpm` workspace packages) |
| Dashboard | React + Vite + Tailwind |
| CLI | Rust |

## Planned repo layout

```
Stardex/
├── ingestor/              # Rust workspace — core engine + decoders + CLI
│   └── crates/
│       ├── core/          # ingestion engine: cursors, reorg, backfill
│       ├── decoders/      # per-contract event decoders
│       └── cli/           # `stardex` command-line tool
├── api/                   # GraphQL + REST server (TypeScript)
├── packages/
│   ├── types/             # shared TS types (@stardex/types)
│   └── sdk/               # typed client (@stardex/sdk)
├── frontend/              # dashboard (React + Vite)
├── db/                    # Postgres schema + migrations
└── docs/                  # guides, decoder tutorial, API reference
```

---

## Roadmap

- [ ] **M1 — Core ingestion**: stream events from RPC, persist to Postgres with cursors.
- [ ] **M2 — First decoders**: token/SAC transfers + balances over time.
- [ ] **M3 — API + SDK**: GraphQL/REST endpoints and a typed TS client.
- [ ] **M4 — Dashboard**: browse contracts and events in the browser.
- [ ] **M5 — Decoder ecosystem**: Soroswap, streaming, and a "write your own decoder" guide.
- [ ] **M6 — Ops**: reorg handling, backfill, retention, Docker deploy.

## Contributing

Stardex is built **in the open for the Stellar ecosystem** and is listed on [GrantFox](https://www.grantfox.xyz/) — contributors claim issues, open PRs, and earn USDC rewards on merge.

- 🌱 New here? Look for [`good first issue`](./docs/BACKLOG.md) labels — most are self-contained decoder, API, UI, or docs tasks.
- 🧩 The highest-leverage way to help is **writing a decoder** for a contract you already use.
- See [`docs/BACKLOG.md`](./docs/BACKLOG.md) for the full, labeled issue list.

## License

Apache-2.0.
