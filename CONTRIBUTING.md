# Contributing to stardex

This repo is the Stardex engine: the Rust ingester, decoders and CLI, plus the Postgres schema in `db/migrations`. The general rules for all Stardex repos (claiming issues, PR size, commit style) are in the [org contributing guide](https://github.com/stardexhq/.github/blob/main/CONTRIBUTING.md). This file only covers what is specific to this repo.

Working on the API, the TypeScript client or the web app? Those live in [stardex-backend](https://github.com/stardexhq/stardex-backend), [stardex-sdk](https://github.com/stardexhq/stardex-sdk) and [stardex-frontend](https://github.com/stardexhq/stardex-frontend).

## Setup

```bash
cd ingestor
cargo build
cargo test
cargo run -p stardex-cli -- decoders list
```

Apply the schema to a database with `DATABASE_URL=... scripts/migrate.sh`. It records applied files in `schema_migrations` and only runs new ones, so run it again any time you pull.

Tests that need a database are marked `#[ignore]`. Run them against a local Postgres with the migrations applied:

```bash
DATABASE_URL=postgres://stardex:stardex@localhost:5432/stardex cargo test -- --ignored
```

## Guidelines

- Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` before pushing. CI checks both.
- The core crate stays neutral. Knowledge about a specific contract or event shape belongs in `crates/decoders`, not `crates/core`.
- New storage goes behind a trait with an in-memory implementation for tests and a Postgres implementation, like `CursorStore` and `ContractStore`.
- This repo owns the database schema. Schema changes are a new numbered file in `db/migrations`; never edit a migration that has already been merged. If the backend needs the change, mention the backend issue in your PR.
- The scheduled workflow applies new migrations to the hosted database before it runs, so a merged migration goes live on the next run. Write migrations that are safe to run on a database that already has data.
- Amounts are integers in the asset's smallest unit (`i128` in Rust, `numeric` in Postgres). No floating point.
