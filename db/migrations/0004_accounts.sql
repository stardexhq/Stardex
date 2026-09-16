-- Stellar accounts whose incoming payments Stardex watches. Each active account
-- gets its own stream, following every transfer paid to it in any asset.
-- Removal is a flag flip so recorded payments stay tied to a known account.

create table if not exists accounts (
    address   text primary key,               -- G... account address
    label     text,                           -- optional human name
    active    boolean not null default true,
    added_at  timestamptz not null default now()
);

create index if not exists accounts_active_idx on accounts (active);
