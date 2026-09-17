#!/usr/bin/env bash
#
# Apply any migrations in db/migrations that this database has not had yet.
#
# Applied files are recorded in `schema_migrations`, and each new file runs in
# its own transaction together with that record, so a failed migration leaves
# nothing half applied and is retried on the next run.
#
# Usage: DATABASE_URL=postgres://... scripts/migrate.sh
set -euo pipefail

if [ -z "${DATABASE_URL:-}" ]; then
  echo "migrate: DATABASE_URL must be set" >&2
  exit 2
fi

dir="$(cd "$(dirname "$0")/../db/migrations" && pwd)"
psql_q() { psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -q -X "$@"; }

# Notices like "already exists, skipping" are expected; keep the output quiet.
quiet="set client_min_messages = warning"

psql_q -c "$quiet" -c "create table if not exists schema_migrations (
  name        text primary key,
  applied_at  timestamptz not null default now()
)"

applied="$(psql_q -tA -c "select name from schema_migrations")"

count=0
for file in "$dir"/*.sql; do
  name="$(basename "$file")"
  if grep -qxF "$name" <<<"$applied"; then
    continue
  fi
  echo "migrate: applying $name"
  # -1 wraps the file and the record in a single transaction.
  psql_q -1 -c "$quiet" -f "$file" -c "insert into schema_migrations (name) values ('$name')"
  count=$((count + 1))
done

echo "migrate: $count migration(s) applied, database is up to date"
