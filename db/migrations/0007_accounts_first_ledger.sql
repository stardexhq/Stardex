-- The ledger that was current when an account was first added. Its stream
-- starts there, so payments sent before the watcher first runs (for example
-- between adding the account and the next scheduled job) are still recorded.

alter table accounts
    add column if not exists first_ledger integer;
