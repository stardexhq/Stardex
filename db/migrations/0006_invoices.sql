-- Invoices and how payments are allocated to them.
--
-- Each invoice gets a unique numeric reference. A customer pays by sending to
-- the muxed address built from the business account and that reference, or to
-- the plain account with the reference as a MEMO_ID. Both arrive on the payment
-- as the same number, which is what the reconcile engine matches on.

-- Starts high so small memo IDs other senders happen to use rarely collide.
create sequence if not exists invoice_reference_seq start 100001;

create table if not exists invoices (
    id               bigserial primary key,
    number           text not null unique,     -- human invoice number, e.g. INV-100001
    account          text not null references accounts (address),
    customer_name    text,
    customer_email   text,
    description      text,
    asset            text not null,            -- SEP-11: "native" or "CODE:ISSUER"
    amount           numeric(39, 0) not null check (amount > 0),
    reference        bigint not null unique default nextval('invoice_reference_seq'),
    status           text not null default 'open'
                     check (status in ('open', 'partial', 'paid', 'overpaid', 'cancelled')),
    amount_received  numeric(39, 0) not null default 0,
    due_date         date,
    issued_at        timestamptz not null default now(),
    paid_at          timestamptz,
    cancelled_at     timestamptz
);

alter sequence invoice_reference_seq owned by invoices.reference;

create index if not exists invoices_account_status_idx on invoices (account, status);

-- Which payment went to which invoice. One payment pays at most one invoice.
create table if not exists payment_allocations (
    id          bigserial primary key,
    payment_id  bigint not null unique references payments (id),
    invoice_id  bigint not null references invoices (id),
    amount      numeric(39, 0) not null check (amount >= 0),
    matched_by  text not null check (matched_by in ('reference', 'manual')),
    created_at  timestamptz not null default now()
);

create index if not exists payment_allocations_invoice_idx on payment_allocations (invoice_id);

-- The single place invoice status is decided. Called by the reconcile engine
-- and by the backend after any allocation change, so both always agree.
create or replace function recalc_invoice(target bigint) returns void
language plpgsql as $$
declare
    inv        invoices%rowtype;
    received   numeric(39, 0);
    last_paid  timestamptz;
begin
    select * into inv from invoices where id = target for update;
    if not found then
        return;
    end if;

    select coalesce(sum(a.amount), 0), max(p.closed_at)
      into received, last_paid
      from payment_allocations a
      join payments p on p.id = a.payment_id
     where a.invoice_id = target;

    update invoices
       set amount_received = received,
           status = case
               when inv.status = 'cancelled' then 'cancelled'
               when received = 0 then 'open'
               when received < inv.amount then 'partial'
               when received = inv.amount then 'paid'
               else 'overpaid'
           end,
           paid_at = case when received >= inv.amount then last_paid end
     where id = target;
end;
$$;
