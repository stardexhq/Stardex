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

## Repositories

This repo is the engine. The rest of Stardex lives in separate repos in the [stardexhq](https://github.com/stardexhq) org:

| Repo | What it is |
|---|---|
| **stardex** (this repo) | Rust ingester, decoders and CLI, plus the Postgres schema in `db/migrations` |
| [stardex-backend](https://github.com/stardexhq/stardex-backend) | HTTP API over the database |
| [stardex-sdk](https://github.com/stardexhq/stardex-sdk) | `@stardex/sdk` on npm: typed client and shared types |
| [stardex-frontend](https://github.com/stardexhq/stardex-frontend) | Web app, including the event explorer |

---

## What works today

Stardex is in active development, but the core engine is real and runs against live testnet:

- [x] **Live event streaming** from Stellar RPC. Pages through a contract's events and polls for new ones, with retry/backoff through transient outages.
- [x] **Multi-contract indexing.** Register any number of contracts (`stardex add`) and index them all at once with `stardex run`. Each contract runs on its own task with its own cursor, so one contract failing is isolated and retried without stalling the rest. Adding or removing a contract takes effect on a running indexer, no restart needed.
- [x] **Account watching.** Watch a Stellar address (`stardex accounts add G...`) and `stardex run` records every payment into it, in any asset, including payments sent to its muxed addresses or with a memo. Each one lands in the `payments` table with its amount, asset, sender and reference, ready to be matched to an invoice.
- [x] **Resumable ingestion.** The cursor is persisted to Postgres, so a restart continues exactly where it left off (verified end-to-end on testnet).
- [x] **Real transfer decoding.** Token `transfer` events, including classic payments in the CAP-67 unified format, are decoded from XDR into typed `{ from, to, amount, asset, to_muxed_id }` records. `to_muxed_id` carries the payment's muxed ID or memo, which is what lets a payment be matched to an invoice.
- [x] **Decoded events stored in Postgres.** Each event runs through the decoder registry and is written to the `events` table; events without a decoder yet are kept raw, so nothing is lost.
- [x] **REST API** ([stardex-backend](https://github.com/stardexhq/stardex-backend)). `GET /events` serves the indexed data with filters (`contractId`, `kind`, ledger range) and cursor pagination; `GET /health` reports DB connectivity.
- [x] **Typed SDK** ([stardex-sdk](https://github.com/stardexhq/stardex-sdk)). `@stardex/sdk` on npm wraps the API so apps query indexed events in a few lines.
- [x] **Web app** ([stardex-frontend](https://github.com/stardexhq/stardex-frontend)). A multi-page React site that browses, filters, and paginates indexed events through the SDK against the live API.
- [x] **Streams (webhooks).** Subscribe a URL to a contract or event kind and Stardex posts matching events to it as they are indexed, signed with HMAC-SHA256 and retried with backoff through a durable queue. Push, so apps stop polling.
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
| **API** ([stardex-backend](https://github.com/stardexhq/stardex-backend)) | TypeScript | REST over the indexed data. |
| **SDK / types** ([stardex-sdk](https://github.com/stardexhq/stardex-sdk)) | TypeScript | `@stardex/sdk`, the typed client and shared types. |
| **Web app** ([stardex-frontend](https://github.com/stardexhq/stardex-frontend)) | React | Multi-page site to browse, filter, and paginate indexed events. |

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
# 1. start Postgres, then bring the schema up to date
docker compose up -d
DATABASE_URL=postgres://stardex:stardex@localhost:5432/stardex scripts/migrate.sh

# 2. register the contracts you want to index, then index them all at once
cd ingestor
export DATABASE_URL=postgres://stardex:stardex@localhost:5432/stardex
cargo run -p stardex-cli -- add <CONTRACT_ID>
cargo run -p stardex-cli -- add <ANOTHER_CONTRACT_ID>
cargo run -p stardex-cli -- run
```

`stardex run` indexes every registered contract concurrently and keeps following the registry, so you can `stardex add` or `stardex remove` a contract in another terminal and the running indexer picks it up within a few seconds. Removing keeps everything that contract already indexed; it just stops following it.

```bash
cargo run -p stardex-cli -- contracts list   # what is being indexed
cargo run -p stardex-cli -- remove <CONTRACT_ID>
```

To stream a single contract without registering it, use `stardex index <CONTRACT_ID>` (add `--once` to catch up to the tip and exit, for scheduled jobs). Without `DATABASE_URL` the single-contract `index` still runs; the cursor just stays in memory (won't survive a restart). Stop with Ctrl-C; on the next run it resumes from where it left off.

### Watch an account for incoming payments

Instead of a whole contract, you can follow the payments into one address:

```bash
cargo run -p stardex-cli -- accounts add <G_ADDRESS> --label "My business"
cargo run -p stardex-cli -- run
```

Watching starts from the ledger that was current when you ran `accounts add`, so payments sent before the watcher first runs are still recorded.

For a scheduled setup with no always-on worker, `stardex run --once` catches every stream up to the tip and exits (add `--accounts-only` to skip contract streams). The hosted demo runs `run --once --accounts-only` and then `reconcile --once` every 15 minutes from GitHub Actions.

Every transfer paid to that address is stored as a `transfer` event, and also recorded as a row in `payments` with its amount, asset, sender and reference (the muxed ID or memo, when present). Outgoing transfers and payments to itself are not recorded as payments, and each event is recorded only once, even if ingestion replays it.

```bash
cargo run -p stardex-cli -- payments list --account <G_ADDRESS>
# 2026-09-17T10:03:52Z  5.0000000 XLM  from GDKA...  ref id:100042  tx 8a41...
```

Payments sent to any `M...` address built on it arrive under the base `G...` address with the ID kept, so there is no need to watch muxed addresses separately. Manage watched accounts with `accounts list` and `accounts remove <G_ADDRESS>`.

### Match payments to invoices

Create an invoice for a watched account. Each one gets a reference number, and the customer pays with it either as a muxed address or as a memo ID:

```bash
cargo run -p stardex-cli -- invoices add <G_ADDRESS> 5 --customer "Acme Ltd"
# invoice INV-100001 for 5.0000000 XLM
#   pay to:  MCN4...GUEKMQ
#   or to:   GCN4...LN4LGI with memo ID 100001
```

Run the reconcile engine next to `stardex run`. It matches each recorded payment to its invoice and keeps invoice status up to date:

```bash
cargo run -p stardex-cli -- reconcile          # add --once to match what is waiting and exit
cargo run -p stardex-cli -- invoices list
# INV-100002  partial  3.0000000 of 5.0000000 XLM  ref 100002  (Globex)
# INV-100001  paid     5.0000000 of 5.0000000 XLM  ref 100001  (Acme Ltd)
```

A payment with no usable reference, an unknown reference, the wrong asset, or for a cancelled invoice stays unmatched with a reason. See [`docs/reconciliation.md`](./docs/reconciliation.md) for exactly how matching works.

### Get events pushed to you (Streams)

Rather than polling `/events`, subscribe a URL and Stardex posts each matching event to it as it is indexed:

```bash
# subscribe a webhook; leave a filter out to match anything
cargo run -p stardex-cli -- subscriptions add https://your-app.example/hooks/stardex \
  --contract <CONTRACT_ID> --kind transfer

# run the dispatcher (its own process, so a slow receiver never stalls indexing)
cargo run -p stardex-cli -- streams
```

For a scheduled setup with no always-on worker (the hosted demo indexes on a cron), `stardex streams --once` drains whatever is due and exits, so delivery runs right after each indexing pass.

Each request carries an `x-stardex-signature: sha256=<hmac>` header, an HMAC-SHA256 of the exact request body using the secret printed when the subscription was created. Verify it before trusting a request:

```js
const want = "sha256=" + createHmac("sha256", SECRET).update(rawBody).digest("hex");
```

The body mirrors what `/events` returns:

```jsonc
{
  "deliveryId": "8412",
  "subscriptionId": "3",
  "event": { "id": "1542451", "contractId": "C...", "ledger": 3653428,
             "kind": "transfer", "fields": { "from": "G...", "amount": "5000000" },
             "closedAt": "2026-07-17T14:31:02.000Z" }
}
```

Delivery is **at-least-once**, so treat `deliveryId` as an idempotency key. Any 2xx counts as accepted; anything else is retried with exponential backoff and marked `dead` after 8 attempts. Subscriptions only receive events indexed from the moment the dispatcher first runs, not a replay of existing history. Manage them with `subscriptions list` and `subscriptions remove <id>`.

To serve the indexed data over HTTP, run [stardex-backend](https://github.com/stardexhq/stardex-backend) against the same database, and [stardex-frontend](https://github.com/stardexhq/stardex-frontend) for the web app. Each repo's README has its setup steps.

---

## Tech stack

| Layer | Stack |
|-------|-------|
| Ingestor & decoders | Rust, Stellar RPC, Soroban XDR (`stellar-xdr`) |
| Storage | PostgreSQL + SQL migrations |
| API | TypeScript, Node ([stardex-backend](https://github.com/stardexhq/stardex-backend)) |
| SDK / types | TypeScript, `@stardex/sdk` on npm ([stardex-sdk](https://github.com/stardexhq/stardex-sdk)) |
| Web app | React + Vite + Tailwind ([stardex-frontend](https://github.com/stardexhq/stardex-frontend)) |
| Infra | Docker Compose |

## Repo layout

```text
stardex/
├── ingestor/              # Rust workspace: engine + decoders + CLI
│   └── crates/
│       ├── core/          # ingestion engine: RPC stream, cursors, streams
│       ├── decoders/      # per-contract event decoders
│       └── cli/           # `stardex` command-line tool
├── db/migrations/         # Postgres schema + migrations
├── docker-compose.yml     # one-command local Postgres
└── .github/workflows/     # CI and the scheduled ingest job
```

---

## Roadmap

- [x] **M1: Core ingestion.** Stream events from RPC, persist a resumable cursor to Postgres.
- [ ] **M2: Decoders**
  - [x] token/SAC transfers
  - [x] CAP-67 classic payments with asset, muxed ID and memo
  - [ ] mint/burn, swaps, payment streams, balances over time
- [ ] **M3: Storage + API**
  - [x] store decoded events in Postgres
  - [x] REST `/events` with filters + cursor pagination
  - [ ] GraphQL endpoint and more REST endpoints
- [x] **M4: SDK + dashboard.** Typed `@stardex/sdk` client and a multi-page React UI to explore indexed events.
- [ ] **M5: Decoder ecosystem.** Soroswap & streaming decoders, plus a "write your own decoder" guide.
- [ ] **M6: Ops.** Reorg handling, backfill, retention policy, Docker deploy.
- [ ] **M8: Streams.** Push indexed events to subscriber webhooks.
  - [x] subscriptions, durable delivery queue, retries with backoff, HMAC-signed payloads (`stardex streams`)
  - [ ] authenticated HTTP API to manage subscriptions, plus SDK and dashboard support
  - [ ] replay a dead delivery, and filter by account as well as contract/kind
- [ ] **M7: Multi-contract indexing service.**
  - [x] register contracts and index them concurrently, isolated per contract (`stardex add` / `run`)
  - [x] auto-recover a contract whose cursor falls behind the RPC retention window
  - [x] add/remove contracts at runtime without a restart (`stardex add` / `remove`)
  - [x] watch accounts for incoming payments alongside contracts (`stardex accounts add`)
  - [x] record incoming payments with their memo or muxed ID (`payments` table, `stardex payments list`)
- [ ] **M9: Payment reconciliation.**
  - [x] invoices with a unique reference, paid by muxed address or memo (`stardex invoices add`)
  - [x] match payments to invoices: paid, partial, overpaid, or unmatched with a reason (`stardex reconcile`)
  - [ ] invoice, payment and export endpoints in [stardex-backend](https://github.com/stardexhq/stardex-backend), plus manual matching
  - [ ] value each payment in a home currency at the time it arrived
  - [ ] journal entries and sync to Xero and QuickBooks

## Contributing

Stardex is built **in the open for the Stellar ecosystem**. Contributions are welcome, claim an issue, open a PR, and it ships on merge.

- **New here?** Look for [`good first issue`](https://github.com/stardexhq/Stardex/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22). Most are self-contained decoder, API, UI, or docs tasks.
- **Highest leverage:** write a **decoder** for a contract you already use; the `token` decoder is a working reference to copy.
- Browse all open work in [Issues](https://github.com/stardexhq/Stardex/issues).

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for setup in this repo, and the [org contributing guide](https://github.com/stardexhq/.github/blob/main/CONTRIBUTING.md) for the rules that apply to every Stardex repo.

## License

[Apache-2.0](./LICENSE).
