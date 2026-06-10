# Stardex Dashboard

The web dashboard for browsing data Stardex has indexed — built with
**React 19 + Vite + Tailwind v4**. It reads from a running Stardex API through
the typed [`@stardex/sdk`](../packages/sdk) client.

## Run it

From the repo root, install workspace dependencies once:

```bash
pnpm install
```

Then start the dev server:

```bash
pnpm --filter @stardex/frontend dev
```

The dashboard expects the API at `http://localhost:8080` (see
[`api/`](../api)). Point it elsewhere with an env var:

```bash
VITE_STARDEX_API=https://my-stardex-api.example pnpm --filter @stardex/frontend dev
```

## Scripts

```bash
pnpm --filter @stardex/frontend dev        # dev server
pnpm --filter @stardex/frontend build      # typecheck + production build
pnpm --filter @stardex/frontend preview    # preview the production build
pnpm --filter @stardex/frontend lint       # eslint
pnpm --filter @stardex/frontend typecheck  # tsc, no emit
```

## What's here

- **Event explorer** — a filterable (by contract ID and event kind), paginated
  table of indexed events, in [`src/components/EventExplorer.tsx`](src/components/EventExplorer.tsx).

## Good first issues

The explorer is the foundation; these views build on it and are open for
contributors (see [`docs/BACKLOG.md`](../docs/BACKLOG.md), milestone M4):

- **Contract list view** — what's indexed and its sync status.
- **Contract detail + chart** — a per-contract page with volume over time.
- **Polish** — richer empty/loading/error states and responsive layout.
