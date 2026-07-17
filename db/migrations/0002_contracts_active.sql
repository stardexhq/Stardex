-- Let a contract be taken out of indexing without losing what it already
-- indexed. `events` references `contracts`, so removal is a flag flip rather
-- than a delete: the rows stay queryable, the ingestor just stops following it.

alter table contracts
    add column if not exists active boolean not null default true;

create index if not exists contracts_active_idx on contracts (active);
