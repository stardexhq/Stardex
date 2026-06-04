# 🗂️ Stardex — Contributor Backlog & Roadmap

Welcome! This page is the map of everything that needs building in Stardex. If
you're looking for something to work on, you're in the right place.

## New here? Read this first

**What Stardex is:** an open-source tool that watches the Stellar/Soroban
blockchain, records every contract event into a normal database, and serves it
back through an API and a dashboard — so any app can ask "what happened in the
past?" without building its own indexer. (Full explanation in the
[README](../README.md).)

**How the project is built** — five connected parts, each its own folder:

| Part | Folder | What it does | Language |
|------|--------|--------------|----------|
| **Ingestor (core)** | `ingestor/crates/core` | Watches the chain, remembers its place | Rust |
| **Decoders** | `ingestor/crates/decoders` | Translate raw events into readable records | Rust |
| **Storage** | `db/` | Where the data lives | Postgres / SQL |
| **API** | `api/` | Answers questions over the web | TypeScript |
| **SDK + types** | `packages/` | The easy button other apps use | TypeScript |
| **Dashboard** | `frontend/` | A website to browse the data | React |
| **CLI** | `ingestor/crates/cli` | The `stardex` command you run | Rust |

Data flows one way: **chain → ingestor → decoders → storage → API → SDK/dashboard.**

**How contributing works:** Stardex is listed on
[GrantFox](https://www.grantfox.xyz/). You claim a GitHub issue, open a pull
request, and earn USDC when it's merged. Comment on an issue to get it assigned
before you start.

## How to read this backlog

Every issue is tagged so you can find work that fits you:

- **Area** (`area:decoders`, `area:api`, …) — which part of the project.
- **Difficulty:**
  - 🟢 `good first issue` — self-contained, great for your first contribution. No deep prior knowledge needed.
  - 🟡 `intermediate` — needs some context, but nothing exotic.
  - 🔴 `advanced` — the hard core (the live ingestion engine). Best for experienced contributors or the maintainer.
- **Type** — `feature`, `bug`, `test`, `docs`, `chore`.

> **The easiest and most valuable place to start is a decoder** (in M2). Each one
> is small, self-contained, and you copy an existing example — yet decoders are
> the heart of what makes Stardex useful.

---

## ✅ Already built (don't re-do these)

The initial scaffold already delivered these, so they're **not** open issues:

- The decoder system itself (the `Decoder` trait + registry) — so you can *add* decoders today.
- The first database schema (`contracts`, `events`, `cursors` tables).
- The API server skeleton (`/health` + an empty `/events`).
- The shared TypeScript types package (`@stardex/types`).
- Continuous Integration (build/test/lint on every PR).

Everything below is genuinely open.

---

## 🛤️ Roadmap (milestones)

Work is grouped into six milestones. They build on each other, but **most issues
can be started right now** — only the M1 engine needs to exist before things can
be tested fully end-to-end.

### 🔴 M1 — Core ingestion (the engine)

**Goal:** make Stardex actually pull live events off the chain and save them.
This is the hard core; expect Rust + async + RPC work.

- **Connect to RPC and stream new events** — the heartbeat: subscribe to a contract and receive its events as they happen.
- **Persist the cursor** — remember exactly where we stopped, so a restart resumes cleanly instead of re-reading or skipping.
- **Detect reorgs and roll back** — if the chain reorganizes, remove events that no longer exist so our data stays truthful.
- **Backfill history** — index a contract's *past* events starting from an older ledger, not just new ones.
- **Integration test harness** — run the ingestor against a testnet RPC in tests.

### 🟢🟡 M2 — Decoders (the contribution engine)

**Goal:** teach Stardex to understand specific contracts. **This is where most
contributions happen** — each decoder is a small, independent PR. Copy the
example `TokenDecoder` and follow the tutorial.

- **Token/SAC transfer decoder** — turn `transfer` events into "who sent what to whom."
- **Token mint / burn / clawback decoder** — the other common token events.
- **Balance-over-time** — derive running balances from transfer history.
- **Soroswap swap/liquidity decoder** — decode DEX swaps and liquidity changes.
- **Payment-streaming decoder** — decode stream create / withdraw / cancel.
- **Generic fallback decoder** — store raw decoded values for contracts we don't specifically support yet.
- **Tests + fixtures** — realistic sample events so decoders are provably correct.

### 🟢🟡 M3 — API & SDK (serving the data)

**Goal:** make the indexed data easy to query — over REST, GraphQL, and a typed
client library.

- **Connect the API to Postgres** — so endpoints read real data.
- **List a contract's events** (paginated, filterable).
- **Events by account/address** — everything involving one wallet.
- **GraphQL schema + resolvers** — a flexible query layer alongside REST.
- **Pagination + filtering helpers** — shared building blocks.
- **Expand the SDK client** — grow it beyond the current `events()` skeleton.
- **SDK quickstart example** — "query a user's history in 5 lines."

### 🟢🟡 M4 — Dashboard (seeing the data)

**Goal:** a website to browse indexed contracts and events without writing code.
Great for frontend contributors.

- **Scaffold the dashboard** — set up the React + Vite + Tailwind app.
- **Contract list view** — what's indexed, and its status.
- **Event explorer** — a filterable, paginated table of events.
- **Contract detail + chart** — a per-contract page with volume over time.
- **Polish** — empty/loading/error states and responsive layout.

### 🟢🟡 M5 — CLI & docs (the developer experience)

**Goal:** make Stardex pleasant to run and easy to learn.

- **`stardex index <contract>`** — wire the command to real ingestion.
- **`stardex new-decoder`** — scaffold a new decoder from a template.
- **Decoder tutorial** — "write your own decoder in 15 minutes."
- **Architecture overview + diagram.**
- **API reference** (REST + GraphQL).

### 🟡 M6 — Ops & deploy (running it for real)

**Goal:** make Stardex easy to run anywhere.

- **Docker + docker-compose** — one command to start ingestor + Postgres + API.
- **Retention policy** — optionally prune events older than a chosen age.
