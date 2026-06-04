#!/usr/bin/env bash
#
# Create Stardex's starter issues on GitHub from the backlog, with full
# descriptions (context / task / pointers / acceptance criteria).
#
# Requires the GitHub CLI (`gh`) installed + authenticated, run from inside the
# repo, with labels created first:
#     bash scripts/setup-labels.sh
#     bash scripts/create-issues.sh
#
# Items already delivered by the initial scaffold (decoder trait, schema v1,
# API skeleton, @stardex/types, CI) are intentionally omitted.
#
# Event-doc links are included. The streaming-decoder issue points at the
# target contract's own source (e.g. your soroban-paystream / StellarPulse).
set -euo pipefail

issue() { gh issue create --title "$1" --label "$2" --body "$3"; }

# ════════════════════════════════════════════════════════════════════
#  M1 — Core ingestion (the engine)
# ════════════════════════════════════════════════════════════════════

issue "feat(ingestor): connect to Stellar RPC and stream new events" \
  "area:ingestor,advanced,feature" \
  "$(cat <<'EOF'
**Context**
This is the heartbeat of Stardex: the core engine that subscribes to a
contract on Stellar RPC and receives its events as they happen. Everything
else (decoders, API, dashboard) depends on this producing a stream of events.

**Task**
Implement live event streaming in the ingestion core — connect to an RPC
endpoint, request events for a given contract, and pass each one into the
decoding/storage pipeline.

**Pointers**
- `ingestor/crates/core/src/lib.rs` — flesh out `Ingestor::index_contract`, currently a stub returning `NotImplemented`.
- The RPC URL is already read from `STARDEX_RPC_URL` (see the CLI).
- You'll likely add an async runtime (e.g. `tokio`) and an HTTP/RPC client — add them to `ingestor/crates/core/Cargo.toml`.
- Stellar RPC `getEvents` reference: https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/getEvents
- Background — building an indexer + contract events: https://developers.stellar.org/docs/build/guides/events

**Acceptance criteria**
- [ ] Given a contract id, the ingestor connects and yields its events
- [ ] Events are surfaced as `RawEvent`s into the pipeline
- [ ] Errors (network, bad contract) are handled gracefully, not panics
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🔴 Advanced — this is the hard core. Pairs closely with the cursor issue.
EOF
)"

issue "feat(ingestor): persist ingestion cursor so it resumes after restart" \
  "area:ingestor,area:db,advanced,feature" \
  "$(cat <<'EOF'
**Context**
If the ingestor stops and restarts, it must continue exactly where it left
off — not re-process old events or skip new ones. A "cursor" records the last
position processed.

**Task**
Persist and restore the ingestion cursor using the `cursors` table.

**Pointers**
- `ingestor/crates/core/src/lib.rs` — the `Cursor` struct already exists.
- `db/migrations/0001_init.sql` — the `cursors` table is already defined.
- Load the cursor on startup; save it as events are processed (batched is fine).

**Acceptance criteria**
- [ ] Cursor is read from Postgres on startup
- [ ] Cursor advances and is persisted as events are processed
- [ ] Restarting resumes from the saved position (covered by a test)
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🔴 Advanced. Depends on the RPC streaming issue.
EOF
)"

issue "feat(ingestor): detect reorgs and roll back orphaned events" \
  "area:ingestor,advanced,feature" \
  "$(cat <<'EOF'
**Context**
Occasionally the chain reorganizes and some ledgers are replaced. If we already
stored events from an orphaned ledger, our data would be wrong. We need to
detect this and roll those events back.

**Task**
Detect ledger reorganizations during ingestion and remove (or supersede) events
from orphaned ledgers so stored data always matches the canonical chain.

**Pointers**
- Builds on the streaming + cursor work in `ingestor/crates/core`.
- Strategy: track recent ledger hashes; on mismatch, delete events at/after the divergence point and rewind the cursor.

**Acceptance criteria**
- [ ] A simulated reorg removes the orphaned events
- [ ] The cursor rewinds to the correct point
- [ ] Test covers the reorg path
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🔴 Advanced — correctness-critical.
EOF
)"

issue "feat(ingestor): backfill a contract's history from a past ledger" \
  "area:ingestor,advanced,feature" \
  "$(cat <<'EOF'
**Context**
Live streaming only captures events from *now* onward. To answer "what happened
over the last 6 months," we also need to index a contract's *past* events.

**Task**
Add a backfill mode that indexes historical events for a contract starting from
a given past ledger, up to the present, then hands off to live streaming.

**Pointers**
- `ingestor/crates/core/src/lib.rs`.
- Page backwards/forwards through historical events via RPC; respect rate limits.
- Reuse the same decode/store path as live ingestion.

**Acceptance criteria**
- [ ] Backfill indexes historical events from a start ledger to head
- [ ] Backfill then transitions cleanly into live streaming (no gap, no dupes)
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🔴 Advanced. Depends on streaming + cursor.
EOF
)"

issue "test(ingestor): integration test harness against testnet RPC" \
  "area:ingestor,intermediate,test" \
  "$(cat <<'EOF'
**Context**
We need confidence the ingestor works against a real RPC, not just unit tests.

**Task**
Set up an integration test harness that runs the ingestor against a local or
testnet RPC and asserts events are ingested and stored correctly.

**Pointers**
- Add an `ingestor/crates/core/tests/` integration test (gate behind a feature or env var so it doesn't run in normal CI).
- Document how to run it in the test file / CONTRIBUTING.

**Acceptance criteria**
- [ ] A documented integration test ingests events from a test RPC
- [ ] It's skippable/offline-safe so default `cargo test` stays green
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟡 Intermediate.
EOF
)"

# ════════════════════════════════════════════════════════════════════
#  M2 — Decoders (great first issues)
# ════════════════════════════════════════════════════════════════════

issue "feat(decoders): real Token/SAC transfer decoder" \
  "area:decoders,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
A decoder turns a raw contract event into a clean, typed record. The repo ships
an example `TokenDecoder` that only recognizes a `transfer` by its topic — it
doesn't yet extract the real data. Let's make it real.

**Task**
Properly decode SAC/token `transfer` events into `from`, `to`, and `amount`
from the event topics + data (XDR), instead of the placeholder fields.

**Pointers**
- `ingestor/crates/decoders/src/lib.rs` — extend the existing `TokenDecoder`.
- Implement the `Decoder` trait (already defined there).
- SAC token event reference: https://developers.stellar.org/docs/tokens/stellar-asset-contract
- SEP-41 token interface: https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md
- CAP-67 (transfer/mint/burn/clawback events): https://github.com/stellar/stellar-protocol/blob/master/core/cap-0067.md

**Acceptance criteria**
- [ ] `transfer` events decode into `from`, `to`, `amount` fields
- [ ] Unit tests with realistic sample event fixtures
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟢 Good first issue — the example is right there to learn from. See the decoder
tutorial (separate issue) once it lands.
EOF
)"

issue "feat(decoders): Token/SAC mint, burn, and clawback decoder" \
  "area:decoders,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
Beyond transfers, tokens emit `mint`, `burn`, and `clawback` events. Indexing
these lets apps track supply changes and admin actions.

**Task**
Add decoding for `mint`, `burn`, and `clawback` events on SAC/token contracts.

**Pointers**
- `ingestor/crates/decoders/src/lib.rs` — add to the token decoder (or a sibling).
- Mirror the structure of the `transfer` decoder.
- SAC token event reference: https://developers.stellar.org/docs/tokens/stellar-asset-contract
- CAP-67 (transfer/mint/burn/clawback events): https://github.com/stellar/stellar-protocol/blob/master/core/cap-0067.md

**Acceptance criteria**
- [ ] `mint`, `burn`, `clawback` decode into typed records with the relevant fields
- [ ] Unit tests with sample fixtures for each
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟢 Good first issue.
EOF
)"

issue "feat(decoders): derive balance-over-time from transfer events" \
  "area:decoders,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
Once transfers are decoded, we can compute how each address's balance changed
over time — a very common query for dashboards and apps.

**Task**
Derive running balances per address from the stream of decoded transfer (and
mint/burn) events.

**Pointers**
- Builds on the transfer + mint/burn decoders.
- Decide where this lives: a derived view in the decoders layer or a materialized table in `db/`. Propose your approach in the issue.

**Acceptance criteria**
- [ ] Given a sequence of transfers, per-address balances are correct over time
- [ ] Tested against a worked example
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟡 Intermediate — a bit of design judgment needed.
EOF
)"

issue "feat(decoders): Soroswap swap/liquidity event decoder" \
  "area:decoders,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
Soroswap is a DEX on Stellar. Decoding its events lets apps query swap volume
and liquidity changes.

**Task**
Implement a decoder for Soroswap `swap`, `deposit` (add liquidity), and
`withdraw` (remove liquidity) events.

**Pointers**
- `ingestor/crates/decoders/src/lib.rs` — implement the `Decoder` trait; copy `TokenDecoder` as a template.
- Register it in `default_registry()`.
- Soroswap docs: https://docs.soroswap.finance/
- Soroswap contract source (events are the source of truth): https://github.com/soroswap/core

**Acceptance criteria**
- [ ] swap / deposit / withdraw decode into typed records with the right fields
- [ ] Unit tests with sample fixtures
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟢 Good first issue.
EOF
)"

issue "feat(decoders): payment-streaming event decoder" \
  "area:decoders,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
Payment-streaming contracts (pay-by-the-second) emit lifecycle events. Indexing
them lets apps show stream history.

**Task**
Implement a decoder for `create`, `withdraw`, and `cancel` events of a
payment-streaming contract.

**Pointers**
- `ingestor/crates/decoders/src/lib.rs` — implement the `Decoder` trait; mirror `TokenDecoder`.
- Register it in `default_registry()`.
- Streaming contract event reference: use the target contract's source — events are defined by its `env.events().publish(...)` calls (e.g. your own soroban-paystream / StellarPulse streaming contract).

**Acceptance criteria**
- [ ] create / withdraw / cancel decode into typed records
- [ ] Unit tests with sample fixtures
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟢 Good first issue.
EOF
)"

issue "feat(decoders): generic fallback decoder for unknown contracts" \
  "area:decoders,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
We can't write a specific decoder for every contract. A generic fallback ensures
we still capture *something* useful for contracts we don't yet support.

**Task**
Add a fallback decoder that stores the raw decoded ScVal (topics + data) in a
structured-but-generic form for any event no specific decoder handled.

**Pointers**
- `ingestor/crates/decoders/src/lib.rs`.
- Should run only when no specific decoder matched — coordinate with the `Registry` dispatch.

**Acceptance criteria**
- [ ] Unmatched events are stored generically (no data lost)
- [ ] Specific decoders still take precedence
- [ ] Unit tests
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟡 Intermediate.
EOF
)"

issue "test(decoders): unit tests + sample fixtures for each decoder" \
  "area:decoders,good first issue,test" \
  "$(cat <<'EOF'
**Context**
Decoders are easy to get subtly wrong. Good fixtures (real-shaped sample events)
make them provably correct and protect against regressions.

**Task**
Add or strengthen unit tests with realistic sample event fixtures across the
existing decoders.

**Pointers**
- `ingestor/crates/decoders/src/lib.rs` — see the existing `#[cfg(test)]` module for the pattern.
- Consider a small `fixtures/` set of sample events.

**Acceptance criteria**
- [ ] Each decoder has tests covering its event kinds + a non-match case
- [ ] Fixtures resemble real event data
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟢 Good first issue — a great way to learn the decoders by testing them.
EOF
)"

# ════════════════════════════════════════════════════════════════════
#  M3 — API & SDK
# ════════════════════════════════════════════════════════════════════

issue "feat(api): connect the API to Postgres" \
  "area:api,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
The API skeleton currently returns an empty list. It needs a real database
connection to serve indexed data.

**Task**
Wire the API server to Postgres: a connection pool, configuration via env vars,
and a health check that verifies DB connectivity.

**Pointers**
- `api/src/index.ts` — the skeleton with `/health` and `/events`.
- Pick a Postgres client (e.g. `pg`); add it to `api/package.json`.
- Read connection config from env (`DATABASE_URL`).

**Acceptance criteria**
- [ ] API connects to Postgres via a pooled client
- [ ] `/health` reports DB connectivity
- [ ] Config is read from env with sensible defaults
- [ ] `pnpm -r typecheck` passes

**Notes**
🟡 Intermediate — unblocks all the query endpoints.
EOF
)"

issue "feat(api): REST endpoint to list a contract's events (paginated)" \
  "area:api,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
The core query: give me a contract's events, newest-first, with filtering and
pagination.

**Task**
Implement `GET /events` returning a contract's events with cursor pagination and
optional filters (kind, ledger range).

**Pointers**
- `api/src/index.ts` — `/events` already exists but returns an empty `Page`.
- Use the shared `EventQuery` / `Page` types from `@stardex/types`.
- Depends on the "connect API to Postgres" issue.

**Acceptance criteria**
- [ ] `/events?contractId=...` returns matching events
- [ ] Supports `kind`, `fromLedger`, `toLedger`, `limit`, `cursor`
- [ ] Returns a `Page<StardexEvent>` with a working `nextCursor`
- [ ] `pnpm -r typecheck` passes

**Notes**
🟢 Good first issue (once the DB connection exists).
EOF
)"

issue "feat(api): REST endpoint for events by account/address" \
  "area:api,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
Users want "everything involving my wallet," across contracts.

**Task**
Add an endpoint returning events that involve a given account/address (e.g. as
sender or receiver), with pagination.

**Pointers**
- `api/src/index.ts`.
- Requires decoded fields to capture addresses (coordinate with the transfer decoder).
- Reuse the pagination helpers.

**Acceptance criteria**
- [ ] An endpoint returns events involving a given address
- [ ] Paginated and filterable
- [ ] `pnpm -r typecheck` passes

**Notes**
🟢 Good first issue.
EOF
)"

issue "feat(api): GraphQL schema + resolvers for events & contracts" \
  "area:api,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
REST is great for simple cases; GraphQL lets clients ask for exactly the shape
they need. We want both.

**Task**
Add a GraphQL endpoint with a schema + resolvers for `events` and `contracts`,
backed by the same data as REST.

**Pointers**
- `api/src/index.ts`.
- Pick a lightweight GraphQL server library; add it to `api/package.json`.
- Reuse the storage/query layer behind REST.

**Acceptance criteria**
- [ ] GraphQL endpoint serves `events` and `contracts` queries
- [ ] Supports filtering + pagination
- [ ] `pnpm -r typecheck` passes

**Notes**
🟡 Intermediate.
EOF
)"

issue "feat(api): cursor-based pagination + filtering helpers" \
  "area:api,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
Every endpoint needs the same pagination + filtering logic. Build it once.

**Task**
Create shared helpers for encoding/decoding cursors and applying filters, used
by all endpoints (REST + GraphQL).

**Pointers**
- `api/src/` — add a small `pagination.ts` (or similar).
- Cursors should be opaque and stable; document the format.

**Acceptance criteria**
- [ ] Reusable helpers for cursor pagination + filters
- [ ] Unit tested
- [ ] Adopted by at least one endpoint
- [ ] `pnpm -r typecheck` passes

**Notes**
🟡 Intermediate.
EOF
)"

issue "feat(sdk): expand the @stardex/sdk client" \
  "area:sdk,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
The SDK is the "easy button" apps use to query Stardex. Today it has only a
skeleton `events()` method.

**Task**
Grow the client: robust error handling, more query methods (by account, by
contract, single event), and optionally a GraphQL mode.

**Pointers**
- `packages/sdk/src/index.ts` — the current `StardexClient`.
- Types live in `packages/sdk` depends on `@stardex/types`.
- Mirror the API endpoints as they land.

**Acceptance criteria**
- [ ] Additional typed query methods
- [ ] Clear errors on non-2xx responses
- [ ] Docs/JSDoc on public methods
- [ ] `pnpm -r typecheck` passes

**Notes**
🟡 Intermediate.
EOF
)"

issue "docs(sdk): quickstart example — query a user's history in 5 lines" \
  "area:sdk,area:docs,good first issue" \
  "$(cat <<'EOF'
**Context**
A short, copy-pasteable example is the best advertisement for the SDK.

**Task**
Write a quickstart that shows installing the SDK and querying a user's history
in a few lines, with expected output.

**Pointers**
- Add to `docs/` and link from the README + `packages/sdk`.
- Use the real `StardexClient` API.

**Acceptance criteria**
- [ ] A runnable, minimal example with expected output
- [ ] Linked from the README
- [ ] Stays accurate to the current SDK API

**Notes**
🟢 Good first issue.
EOF
)"

# ════════════════════════════════════════════════════════════════════
#  M4 — Dashboard
# ════════════════════════════════════════════════════════════════════

issue "chore(frontend): scaffold the dashboard (React + Vite + Tailwind)" \
  "area:frontend,good first issue,chore" \
  "$(cat <<'EOF'
**Context**
The `frontend/` folder is currently just a placeholder README. The dashboard
needs a real app to build on.

**Task**
Scaffold a React + Vite + Tailwind app wired into the pnpm workspace as
`@stardex/frontend`, with a `dev` script and a typecheck script.

**Pointers**
- `frontend/` — replace the placeholder.
- Add it to the pnpm workspace (already globbed in `pnpm-workspace.yaml`).
- Match the conventions of the other packages (`type: module`, `typecheck` script so CI covers it).

**Acceptance criteria**
- [ ] `pnpm --filter @stardex/frontend dev` starts the app
- [ ] `pnpm -r typecheck` includes the frontend and passes
- [ ] Tailwind is configured and working

**Notes**
🟢 Good first issue — unblocks all other dashboard work.
EOF
)"

issue "feat(frontend): contract list view" \
  "area:frontend,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
The first useful screen: which contracts are indexed and what their status is.

**Task**
Build a view that lists indexed contracts (id, first-seen ledger, status) using
the SDK.

**Pointers**
- Depends on the dashboard scaffold + an API endpoint listing contracts.
- Use `@stardex/sdk`.

**Acceptance criteria**
- [ ] Contracts render in a list/table
- [ ] Loading + empty states handled
- [ ] `pnpm -r typecheck` passes

**Notes**
🟢 Good first issue.
EOF
)"

issue "feat(frontend): event explorer table with filters & pagination" \
  "area:frontend,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
The core browsing experience: a table of decoded events you can filter and page
through.

**Task**
Build an event explorer table with filters (contract, kind, ledger range) and
cursor pagination, backed by the SDK.

**Pointers**
- Depends on the dashboard scaffold + the events endpoint.
- Reuse the SDK's pagination.

**Acceptance criteria**
- [ ] Events render with filtering + pagination
- [ ] Loading/empty/error states
- [ ] `pnpm -r typecheck` passes

**Notes**
🟡 Intermediate.
EOF
)"

issue "feat(frontend): contract detail page with volume-over-time chart" \
  "area:frontend,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
A per-contract page that visualizes activity — e.g. event/volume over time.

**Task**
Build a contract detail page including a volume-over-time chart and recent
events.

**Pointers**
- Depends on the scaffold + relevant endpoints.
- Pick a lightweight charting lib.

**Acceptance criteria**
- [ ] Detail page shows contract info + a time-series chart
- [ ] Handles loading/empty/error
- [ ] `pnpm -r typecheck` passes

**Notes**
🟡 Intermediate.
EOF
)"

issue "feat(frontend): empty/loading/error states + responsive layout" \
  "area:frontend,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
Polish that makes the dashboard feel finished and usable on any screen.

**Task**
Add consistent empty, loading, and error states across views, and make the
layout responsive.

**Pointers**
- Depends on the scaffold + at least one view existing.
- Consider small shared components for these states.

**Acceptance criteria**
- [ ] Consistent loading/empty/error UI across views
- [ ] Layout works on mobile + desktop
- [ ] `pnpm -r typecheck` passes

**Notes**
🟢 Good first issue.
EOF
)"

# ════════════════════════════════════════════════════════════════════
#  M5 — CLI & docs
# ════════════════════════════════════════════════════════════════════

issue "feat(cli): wire \`stardex index <contract>\` to real ingestion" \
  "area:cli,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
The CLI's `index` command currently calls a stubbed ingestor. Once the engine
works, the command should actually run it.

**Task**
Connect `stardex index <contract>` to the real ingestion engine, with basic
flags (e.g. `--from-ledger` for backfill) and useful output.

**Pointers**
- `ingestor/crates/cli/src/main.rs`.
- Depends on M1 (streaming/backfill).

**Acceptance criteria**
- [ ] `stardex index <contract>` runs real ingestion
- [ ] Sensible logging/progress output
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟡 Intermediate. Depends on M1.
EOF
)"

issue "feat(cli): \`stardex new-decoder\` scaffold command" \
  "area:cli,good first issue,feature" \
  "$(cat <<'EOF'
**Context**
Writing decoders is the most common contribution. A scaffolder lowers the
barrier even further.

**Task**
Add a `stardex new-decoder <name>` command that generates a new decoder module
from a template (struct + `Decoder` impl + a test stub) and prints next steps.

**Pointers**
- `ingestor/crates/cli/src/main.rs` — add the subcommand.
- Base the template on the existing `TokenDecoder`.

**Acceptance criteria**
- [ ] Command generates a compiling decoder skeleton + test stub
- [ ] Prints how to register it
- [ ] `cargo fmt` + `cargo clippy` pass

**Notes**
🟢 Good first issue.
EOF
)"

issue "docs: tutorial — write your own decoder in 15 minutes" \
  "area:docs,good first issue" \
  "$(cat <<'EOF'
**Context**
Decoders are the #1 way to contribute. A friendly tutorial turns curious
visitors into contributors.

**Task**
Write a step-by-step tutorial that walks a newcomer through adding a decoder,
using the example `TokenDecoder`, ending with a passing test.

**Pointers**
- Add to `docs/`; link from README + CONTRIBUTING.
- Reference `ingestor/crates/decoders/src/lib.rs`.

**Acceptance criteria**
- [ ] A complete, accurate walkthrough from zero to a tested decoder
- [ ] Linked from README + CONTRIBUTING
- [ ] Beginner-friendly tone

**Notes**
🟢 Good first issue and high-leverage for the whole project.
EOF
)"

issue "docs: architecture overview + diagram" \
  "area:docs,good first issue" \
  "$(cat <<'EOF'
**Context**
A clear architecture doc helps contributors understand how the pieces fit.

**Task**
Write an architecture overview explaining the engine → decoders → storage → API
→ SDK/dashboard flow, with a diagram.

**Pointers**
- Add to `docs/`; the README already has a starter ASCII diagram to expand on.

**Acceptance criteria**
- [ ] Doc explains each component + the data flow
- [ ] Includes a diagram
- [ ] Linked from README

**Notes**
🟢 Good first issue.
EOF
)"

issue "docs: API reference (REST + GraphQL)" \
  "area:docs,good first issue" \
  "$(cat <<'EOF'
**Context**
Developers need a reference for the endpoints Stardex exposes.

**Task**
Document the REST endpoints and the GraphQL schema, with request/response
examples.

**Pointers**
- Add to `docs/`; keep it in sync with `api/`.
- Depends on the relevant endpoints existing.

**Acceptance criteria**
- [ ] Each endpoint/field documented with examples
- [ ] Linked from README

**Notes**
🟢 Good first issue (best done as endpoints stabilize).
EOF
)"

# ════════════════════════════════════════════════════════════════════
#  M6 — Ops & deploy
# ════════════════════════════════════════════════════════════════════

issue "feat: Dockerfile + docker-compose (ingestor + Postgres + API)" \
  "area:ingestor,area:db,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
Running Stardex should be one command. A docker-compose stack makes local setup
and deployment trivial.

**Task**
Add Dockerfiles for the ingestor and API, plus a `docker-compose.yml` that
brings up ingestor + Postgres + API together, with migrations applied.

**Pointers**
- Root of the repo.
- Apply `db/migrations/` on startup.
- Document usage in README.

**Acceptance criteria**
- [ ] `docker compose up` starts ingestor + Postgres + API
- [ ] Migrations run automatically
- [ ] Documented in README

**Notes**
🟡 Intermediate.
EOF
)"

issue "feat(db): configurable retention policy" \
  "area:db,intermediate,feature" \
  "$(cat <<'EOF'
**Context**
Some operators won't want to keep events forever. A retention policy lets them
prune old data.

**Task**
Add an optional, configurable retention policy that prunes events older than a
chosen age/threshold.

**Pointers**
- `db/` + the ingestor/API config.
- Make it opt-in and clearly documented; default to "keep everything."

**Acceptance criteria**
- [ ] Configurable retention threshold
- [ ] Pruning runs safely on a schedule
- [ ] Off by default; documented

**Notes**
🟡 Intermediate.
EOF
)"

echo "✅ Issues created."
