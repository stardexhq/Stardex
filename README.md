# Stardex

> **The open-source indexer & data API for Stellar / Soroban: record every contract event, query any moment in history.**

[![Status](https://img.shields.io/badge/status-active%20development-orange)](#roadmap)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](./LICENSE)
[![Built on Stellar](https://img.shields.io/badge/built%20on-Stellar%20%2F%20Soroban-7B5BFF)](https://stellar.org)
[![Rust](https://img.shields.io/badge/core-Rust-DEA584)](#tech-stack)
[![Postgres](https://img.shields.io/badge/store-PostgreSQL-336791)](#tech-stack)

**Live demo:** [stardex.onrender.com](https://stardex.onrender.com) (browse real testnet events in the explorer).
**Public API:** [stardex-api.onrender.com](https://stardex-api.onrender.com), try [`/events?limit=5`](https://stardex-api.onrender.com/events?limit=5) or [`/health`](https://stardex-api.onrender.com/health).



Stellar's RPC only keeps a short window of history and then prunes it. Stardex captures that history durably and makes it queryable, so any dApp can ask *"every payment this user has made,"* *"this contract's daily volume,"* or *"every swap this AMM has emitted,"* without rebuilding indexing from scratch.

---

## What works today

Stardex is in active development, but the core engine is real and runs against live testnet:

- [x] **Live event streaming** from Stellar RPC. Pages through a contract's events and polls for new ones, with retry/backoff through transient outages.
- [x] **Multi-contract indexing.** Register any number of contracts (`stardex add`) and index them all at once with `stardex run`. Each contract runs on its own task with its own cursor, so one contract failing is isolated and retried without stalling the rest.
- [x] **Resumable ingestion.** The cursor is persisted to Postgres, so a restart continues exactly where it left off (verified end-to-end on testnet).
- [x] **Real transfer decoding.** SAC / token `transfer` events are decoded from XDR into typed `{ from, to, amount }` records.
- [x] **Decoded events stored in Postgres.** Each event runs through the decoder registry and is written to the `events` table; events without a decoder yet are kept raw, so nothing is lost.
- [x] **REST API.** `GET /events` serves the indexed data with filters (`contractId`, `kind`, ledger range) and cursor pagination; `GET /health` reports DB connectivity.
- [x] **Typed SDK.** `@stardex/sdk` (`StardexClient`) wraps the API so apps query indexed events in a few lines.
- [x] **Web dashboard.** A multi-page React site that browses, filters, and paginates indexed events through the SDK against the live API.
- [ ] **In progress.** The GraphQL API and more decoders (mint/burn, swaps, payment streams).

```text
$ stardex index CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC
storage: Postgres (events + resumable cursor)
indexing CDLZ... via https://soroban-testnet.stellar.org ...
starting from current ledger 2932199

# events land in Postgres, decoded where a decoder exists:
kind     | fields
---------+-------------------------------------------------------------------
transfer | {"from":"GBSOLM...","to":"GCMLUV...","amount":"5000000"}
raw      | {"topics":["AAAADwAAAANmZWUA", ...],"data":"AAAACv//..."}
```

---

## The problem

Stellar's RPC is built for *recent* data. It keeps only a short window of history and then prunes it. That makes the most common questions in any app **surprisingly hard to answer**:

- "Show me every payment this user has made over the last 6 months."
- "What was this contract's volume, day by day?"
- "List all the streams / swaps / mints this contract has ever emitted."

Today, **every Soroban team rebuilds the same indexing plumbing from scratch**: a service that watches the chain, copies events into a database, and exposes them for querying. It's duplicated effort across the entire ecosystem, with no canonical open-source tool that does it.

## What Stardex is

Stardex is that plumbing, **built once, as a tool everyone can use and self-host.** Point it at a contract and it:

1. **Watches** the Stellar/Soroban chain in real time.
2. **Decodes** each contract's events into clean, typed records.
3. **Stores** them durably in Postgres, so old history is never lost and stays fast to search.
4. **Serves** them through a **GraphQL + REST API**, a typed **TypeScript SDK**, and a **web dashboard**.

---

## Architecture

Stardex is built on one core idea: a **neutral ingestion engine** that knows *how* to read the chain, and **pluggable decoders** that know *what* each contract's events mean. New contract support is a new decoder, never a change to the engine.

```text
   Stellar RPC
   (getEvents)
       │  raw events (XDR)
       ▼
 ┌───────────────┐     ┌───────────────┐     ┌──────────────┐
 │   Ingestor    │     │   Decoders    │     │   Postgres   │
 │    (core)     │────▶│ (per-contract │────▶│    store     │
 │               │     │  translators) │     │              │
 │ • RPC stream  │     │ • token/SAC   │     │ • events     │
 │ • cursors     │     │ • swaps       │     │ • cursors    │
 │ • retry/      │     │ • streams     │     │ • migrations │
 │   backfill    │     │ • your own... │     │              │
 └───────────────┘     └───────────────┘     └──────┬───────┘
       Rust                  Rust                    │
                                                     ▼
                                        ┌─────────────────────────┐
                                        │     API (GraphQL +       │
                                        │     REST), TypeScript    │
                                        └───────┬──────────┬───────┘
                                                ▼          ▼
                                          @stardex/sdk   Dashboard
                                          (typed client) (React)
```

### Design principles

- **Neutral core, pluggable edges.** The ingestor is opinion-free about event meaning; all protocol knowledge lives in decoders. Adding a contract never risks the engine.
- **Resumable by default.** Ingestion persists a cursor every page, so restarts, crashes, and deploys never lose or double-process events.
- **Self-hostable, no exotic dependencies.** Plain Rust, Postgres, TypeScript, React. No ZK, no forked VMs, no special hardware. Clone it and run it.
- **Typed all the way up.** Raw XDR to typed Rust records to Postgres to typed API to `@stardex/sdk`, so apps consume history in a few lines without re-deriving types.

### Components

| Component | Lang | Role |
|-----------|------|------|
| **Ingestor** (`ingestor/crates/core`) | Rust | Streams events from RPC, tracks cursors, handles backfill & reorgs. |
| **Decoders** (`ingestor/crates/decoders`) | Rust | Per-contract XDR to typed rows. **Where most contributions happen.** |
| **CLI** (`ingestor/crates/cli`) | Rust | `stardex add` / `run` to index many contracts, `index` for one, plus decoders & backfills. |
| **Store** (`db/`) | SQL | Postgres schema, migrations, retention. |
| **API** (`api/`) | TypeScript | GraphQL + REST over the indexed data. |
| **SDK / types** (`packages/`) | TypeScript | `@stardex/sdk` + `@stardex/types`, the typed client. |
| **Dashboard** (`frontend/`) | React | Multi-page site to browse, filter, and paginate indexed events. |

### Adding support for a contract = writing a decoder

A decoder turns a raw event into a typed record. That's the whole extension model:

```rust
impl Decoder for TokenDecoder {
    fn name(&self) -> &'static str { "token" }

    fn decode(&self, event: &RawEvent) -> Option<DecodedEvent> {
        // recognize a "transfer", pull out from / to / amount ...
    }
}
```

```jsonc
// in  -> raw event topics/value (base64 XDR)
// out -> a typed record
{ "kind": "transfer", "from": "G...", "to": "G...", "amount": "1000" }
```

---

## Quick start

**Try the live version, no setup:** open the [dashboard](https://stardex.onrender.com), or query the hosted API directly:

```bash
curl "https://stardex-api.onrender.com/events?limit=5"
```

To run your own instance:

> Requires Rust, and Docker (or a local Postgres).

```bash
# 1. start Postgres (auto-applies the schema in db/migrations)
docker compose up -d

# 2. register the contracts you want to index, then index them all at once
cd ingestor
export DATABASE_URL=postgres://stardex:stardex@localhost:5432/stardex
cargo run -p stardex-cli -- add <CONTRACT_ID>
cargo run -p stardex-cli -- add <ANOTHER_CONTRACT_ID>
cargo run -p stardex-cli -- run
```

`stardex run` indexes every registered contract concurrently. To stream a single contract without registering it, use `stardex index <CONTRACT_ID>` (add `--once` to catch up to the tip and exit, for scheduled jobs). Without `DATABASE_URL` the single-contract `index` still runs; the cursor just stays in memory (won't survive a restart). Stop with Ctrl-C; on the next run it resumes from where it left off.

```bash
# 3. serve the indexed data over HTTP (in another terminal)
pnpm install
DATABASE_URL=postgres://stardex:stardex@localhost:5432/stardex \
  node api/src/index.ts

# then query it:
curl "http://localhost:8080/events?kind=transfer&limit=5"
```

---

## Tech stack

| Layer | Stack |
|-------|-------|
| Ingestor & decoders | Rust, Stellar RPC, Soroban XDR (`stellar-xdr`, `stellar-strkey`) |
| Storage | PostgreSQL + SQL migrations |
| API | TypeScript (GraphQL + REST) |
| SDK / types | TypeScript (`pnpm` workspace packages) |
| Dashboard | React + Vite + Tailwind |
| Infra | Docker Compose |

## Repo layout

```text
Stardex/
├── ingestor/              # Rust workspace: engine + decoders + CLI
│   └── crates/
│       ├── core/          # ingestion engine: RPC stream, cursors, backfill
│       ├── decoders/      # per-contract event decoders
│       └── cli/           # `stardex` command-line tool
├── api/                   # GraphQL + REST server (TypeScript)
├── packages/
│   ├── types/             # shared TS types (@stardex/types)
│   └── sdk/               # typed client (@stardex/sdk)
├── frontend/              # dashboard (React + Vite)
├── db/migrations/         # Postgres schema + migrations
├── docker-compose.yml     # one-command local Postgres
└── docs/                  # guides, decoder tutorial, API reference
```

---

## Roadmap

- [x] **M1: Core ingestion.** Stream events from RPC, persist a resumable cursor to Postgres.
- [ ] **M2: Decoders**
  - [x] token/SAC transfers
  - [ ] mint/burn, swaps, payment streams, balances over time
- [ ] **M3: Storage + API**
  - [x] store decoded events in Postgres
  - [x] REST `/events` with filters + cursor pagination
  - [ ] GraphQL endpoint and more REST endpoints
- [x] **M4: SDK + dashboard.** Typed `@stardex/sdk` client and a multi-page React UI to explore indexed events.
- [ ] **M5: Decoder ecosystem.** Soroswap & streaming decoders, plus a "write your own decoder" guide.
- [ ] **M6: Ops.** Reorg handling, backfill, retention policy, Docker deploy.
- [ ] **M7: Multi-contract indexing service.**
  - [x] register contracts and index them concurrently, isolated per contract (`stardex add` / `run`)
  - [x] auto-recover a contract whose cursor falls behind the RPC retention window
  - [ ] add/remove contracts at runtime without a restart

## Contributing

Stardex is built **in the open for the Stellar ecosystem**. Contributions are welcome, claim an issue, open a PR, and it ships on merge.

- **New here?** Look for [`good first issue`](https://github.com/stardexhq/Stardex/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22). Most are self-contained decoder, API, UI, or docs tasks.
- **Highest leverage:** write a **decoder** for a contract you already use; the `token` decoder is a working reference to copy.
- Browse all open work in [Issues](https://github.com/stardexhq/Stardex/issues).

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for setup and conventions.

## License

[Apache-2.0](./LICENSE).
