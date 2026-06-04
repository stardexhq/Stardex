-- Stardex initial schema (backlog issue #5).
-- Minimal v1: contracts we track, the events we decode, and ingestion cursors.

create table if not exists contracts (
    contract_id        text primary key,
    first_seen_ledger  integer not null,
    added_at           timestamptz not null default now()
);

create table if not exists events (
    id           bigserial primary key,
    contract_id  text not null references contracts (contract_id),
    ledger       integer not null,
    kind         text not null,                  -- decoder-defined, e.g. "transfer"
    fields       jsonb not null default '{}'::jsonb,
    closed_at    timestamptz not null            -- ledger close time
);

create index if not exists events_contract_ledger_idx on events (contract_id, ledger);
create index if not exists events_kind_idx on events (kind);

-- Tracks how far ingestion has progressed, so it can resume after a restart.
create table if not exists cursors (
    stream         text primary key,             -- a contract_id, or "global"
    last_ledger    integer not null default 0,
    last_event_id  text,
    updated_at     timestamptz not null default now()
);
