-- Stardex Streams: push indexed events to subscriber webhooks.
--
-- Subscriptions say what to send and where. Deliveries are the durable outbox:
-- one row per (subscription, event) to send, so retries survive a restart and
-- nothing is lost if a receiver is down.

create table if not exists subscriptions (
    id           bigserial primary key,
    url          text not null,
    contract_id  text,                            -- null = any contract
    kind         text,                            -- null = any event kind
    secret       text not null,                   -- HMAC-SHA256 signing secret
    active       boolean not null default true,
    created_at   timestamptz not null default now()
);

create index if not exists subscriptions_active_idx on subscriptions (active);

create table if not exists deliveries (
    id              bigserial primary key,
    subscription_id bigint not null references subscriptions (id),
    event_id        bigint not null references events (id),
    -- pending: still to send. delivered: accepted. dead: gave up after retries.
    status          text not null default 'pending',
    attempts        int not null default 0,
    next_attempt_at timestamptz not null default now(),
    last_error      text,
    delivered_at    timestamptz
);

-- Drives the "what is due to send" query.
create index if not exists deliveries_due_idx on deliveries (status, next_attempt_at);
-- Makes enqueueing idempotent: an event can never be queued twice for the same
-- subscription, so a replayed cursor cannot cause a double send.
create unique index if not exists deliveries_sub_event_idx
    on deliveries (subscription_id, event_id);

-- How far the dispatcher has read through `events`. Separate from the ingestion
-- cursors, which track position in the chain rather than in our own table.
create table if not exists stream_cursors (
    name          text primary key,
    last_event_id bigint not null default 0
);
