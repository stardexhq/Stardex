-- Incoming payments to watched accounts, one row per transfer event. This is
-- what gets matched to invoices. Amounts are raw integer units of the asset
-- (stroops for classic assets), never floating point.

create table if not exists payments (
    id                bigserial primary key,
    event_id          text not null unique,   -- RPC event id; makes recording idempotent
    tx_hash           text not null,
    ledger            integer not null,
    closed_at         timestamptz not null,
    account           text not null references accounts (address),
    from_address      text not null,
    asset             text not null,          -- SEP-11 ("native", "USDC:G...") or token contract id
    asset_contract    text not null,          -- contract that emitted the transfer
    amount            numeric(39, 0) not null check (amount >= 0),
    reference_type    text check (reference_type in ('id', 'text', 'hash')),
    reference         text,                   -- muxed ID or memo, if the payment carried one
    match_status      text not null default 'unmatched'
                      check (match_status in ('unmatched', 'matched', 'ignored')),
    unmatched_reason  text,
    recorded_at       timestamptz not null default now()
);

create index if not exists payments_account_closed_idx on payments (account, closed_at);
create index if not exists payments_match_status_idx on payments (match_status);
create index if not exists payments_account_reference_idx on payments (account, reference);
