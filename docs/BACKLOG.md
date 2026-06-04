# 🗂️ Stardex — Campaign-Ready Issue Backlog

This is the seed backlog for **Stardex**. It's structured for a [GrantFox](https://www.grantfox.xyz/) campaign:
most issues are **approachable** (decoders, API endpoints, UI, docs) so contributors of every tier can
pick something up, with a **small hard core** reserved for experienced contributors / the maintainer.

## How the backlog is balanced

| Bucket | What it is | Who picks it up | Share of backlog |
|--------|-----------|-----------------|------------------|
| 🟢 **`good first issue`** | Self-contained: one decoder, one endpoint, one component, one doc | Anyone, incl. first-timers | ~60% |
| 🟡 **`intermediate`** | Needs context but no deep specialization | Mid-tier contributors | ~30% |
| 🔴 **`core` / `advanced`** | Ingestion engine, reorg/backfill correctness | Maintainer + Tier-4/5 | ~10% |

> The novelty/risk lives in the small 🔴 core; the **issue volume** (and leaderboard activity) comes from the
> large 🟢/🟡 surface. That's the design goal — lots of mergeable work that the maintainer can confidently review.

## Label scheme

- **Area:** `area:ingestor` · `area:decoders` · `area:api` · `area:sdk` · `area:frontend` · `area:db` · `area:cli` · `area:docs`
- **Difficulty:** `good first issue` · `intermediate` · `advanced`
- **Type:** `feature` · `test` · `docs` · `chore`

---

## 🔴 M1 — Core ingestion (the hard core — keep this small)

| # | Title | Labels |
|---|-------|--------|
| 1 | Ingestor skeleton: connect to Stellar RPC and stream new events | `area:ingestor` `advanced` `feature` |
| 2 | Cursor/checkpoint persistence so ingestion resumes where it left off | `area:ingestor` `area:db` `advanced` `feature` |
| 3 | Reorg detection & rollback of orphaned events | `area:ingestor` `advanced` `feature` |
| 4 | Backfill mode: index a contract's history from a past ledger | `area:ingestor` `advanced` `feature` |
| 5 | Postgres schema v1: `events`, `contracts`, `cursors` tables + migrations | `area:db` `intermediate` `feature` |
| 6 | Integration test harness against a local/testnet RPC | `area:ingestor` `intermediate` `test` |

## 🟡🟢 M2 — Decoders (the contribution engine — endless approachable work)

> Each decoder is a small, self-contained PR: take raw event XDR for one contract type, output typed rows.
> Adding a decoder is the canonical `good first issue`.

| # | Title | Labels |
|---|-------|--------|
| 7 | Decoder trait/interface + registry so decoders plug in cleanly | `area:decoders` `intermediate` `feature` |
| 8 | Token/SAC `transfer` event decoder | `area:decoders` `good first issue` `feature` |
| 9 | Token/SAC `mint` / `burn` / `clawback` decoder | `area:decoders` `good first issue` `feature` |
| 10 | Derive "balance over time" from token transfer events | `area:decoders` `intermediate` `feature` |
| 11 | Soroswap swap/liquidity event decoder | `area:decoders` `good first issue` `feature` |
| 12 | Payment-streaming event decoder (create/withdraw/cancel) | `area:decoders` `good first issue` `feature` |
| 13 | Generic fallback decoder: store raw decoded ScVal for unknown contracts | `area:decoders` `intermediate` `feature` |
| 14 | Unit tests + sample fixtures for each decoder | `area:decoders` `good first issue` `test` |

## 🟡🟢 M3 — API + SDK

| # | Title | Labels |
|---|-------|--------|
| 15 | API server skeleton (health, config, Postgres connection) | `area:api` `intermediate` `feature` |
| 16 | REST endpoint: list events for a contract (paginated, filterable) | `area:api` `good first issue` `feature` |
| 17 | REST endpoint: events for an account/address | `area:api` `good first issue` `feature` |
| 18 | GraphQL schema + resolvers for events & contracts | `area:api` `intermediate` `feature` |
| 19 | Cursor-based pagination + filtering helpers | `area:api` `intermediate` `feature` |
| 20 | `@stardex/types` — shared TS types for events & API responses | `area:sdk` `good first issue` `feature` |
| 21 | `@stardex/sdk` — typed client wrapping the REST/GraphQL API | `area:sdk` `intermediate` `feature` |
| 22 | SDK quickstart example: "query a user's history in 5 lines" | `area:sdk` `area:docs` `good first issue` `docs` |

## 🟢 M4 — Dashboard

| # | Title | Labels |
|---|-------|--------|
| 23 | Dashboard scaffold (React + Vite + Tailwind) | `area:frontend` `good first issue` `chore` |
| 24 | Contract list view (indexed contracts + status) | `area:frontend` `good first issue` `feature` |
| 25 | Event explorer table with filters & pagination | `area:frontend` `intermediate` `feature` |
| 26 | Contract detail page with a volume-over-time chart | `area:frontend` `intermediate` `feature` |
| 27 | Empty/loading/error states + responsive layout | `area:frontend` `good first issue` `feature` |

## 🟡🟢 M5 — CLI & decoder ecosystem

| # | Title | Labels |
|---|-------|--------|
| 28 | `stardex index <contract>` command | `area:cli` `intermediate` `feature` |
| 29 | `stardex decoders list` + scaffold command for a new decoder | `area:cli` `good first issue` `feature` |
| 30 | **Tutorial: "Write your own decoder in 15 minutes"** | `area:docs` `good first issue` `docs` |
| 31 | Architecture overview doc + diagram | `area:docs` `good first issue` `docs` |
| 32 | API reference (REST + GraphQL) | `area:docs` `good first issue` `docs` |

## 🟡 M6 — Ops & deploy

| # | Title | Labels |
|---|-------|--------|
| 33 | Dockerfile + docker-compose (ingestor + Postgres + API) | `area:ingestor` `area:db` `intermediate` `feature` |
| 34 | Configurable retention policy (prune events older than N) | `area:db` `intermediate` `feature` |
| 35 | CI: build, fmt, clippy, test (Rust) + lint/typecheck (TS) | `chore` `intermediate` |

---

### Notes for the maintainer

- This seed list is **~35 issues**; in practice the **decoders** and **docs** areas alone expand to 80–100+
  (one issue per contract type, one per endpoint, one per UI view), giving a near-unlimited contributor supply.
- File the 🔴 `core` issues first and either tackle them yourself or reserve them for trusted contributors —
  the rest of the backlog only needs the engine to exist, not to be perfect.
- Keep a steady stream of `good first issue` decoders open during each campaign; they're the highest-throughput,
  easiest-to-review, most-rewarding contributions.
