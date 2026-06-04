#!/usr/bin/env bash
#
# One-time setup: create Stardex's labels on the GitHub repo.
# Requires the GitHub CLI (`gh`) installed and authenticated.
# Run once from inside the repo:  bash scripts/setup-labels.sh
#
set -euo pipefail

create() { gh label create "$1" --color "$2" --description "$3" --force; }

# --- Areas (which part of the codebase) ---
create "area:ingestor" "1d76db" "Core ingestion engine"
create "area:decoders" "0e8a16" "Per-contract event decoders"
create "area:api"      "5319e7" "HTTP / GraphQL API"
create "area:sdk"      "fbca04" "TypeScript SDK"
create "area:frontend" "d93f0b" "Dashboard UI"
create "area:db"       "006b75" "Database schema / migrations"
create "area:cli"      "c5def5" "Command-line tool"
create "area:docs"     "0075ca" "Documentation"

# --- Difficulty ---
create "good first issue" "7057ff" "Good for newcomers"
create "intermediate"     "a2eeef" "Needs some context, no deep specialization"
create "advanced"         "b60205" "Core / hard issue (maintainer or senior contributor)"

# --- Type ---
create "feature" "0e8a16" "New feature or task"
create "bug"     "d73a4a" "Something isn't working"
create "test"    "fef2c0" "Tests / fixtures"
create "chore"   "ededed" "Tooling, CI, housekeeping"

echo "✅ Labels created/updated."
