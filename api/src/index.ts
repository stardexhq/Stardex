/**
 * Stardex API server — the "front desk" that answers questions over the web.
 * Serves `/health` (with a DB check) and `/events` (filtered, paginated reads
 * from Postgres). TODO(#18): add GraphQL.
 */
import { createServer } from "node:http";
import type { EventQuery } from "@stardex/types";
import { pingDb } from "./db.ts";
import { queryEvents } from "./events.ts";

const PORT = Number(process.env.PORT ?? 8080);

const server = createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", `http://localhost:${PORT}`);
  res.setHeader("content-type", "application/json");

  // Public read-only data API: allow browser dashboards on any origin.
  res.setHeader("access-control-allow-origin", "*");
  res.setHeader("access-control-allow-methods", "GET, OPTIONS");
  res.setHeader("access-control-allow-headers", "content-type");

  if (req.method === "OPTIONS") {
    res.writeHead(204);
    res.end();
    return;
  }

  try {
    if (url.pathname === "/health") {
      const dbOk = await pingDb();
      res.writeHead(dbOk ? 200 : 503);
      res.end(JSON.stringify({ status: dbOk ? "ok" : "degraded", db: dbOk }));
      return;
    }

    if (url.pathname === "/events") {
      const page = await queryEvents(parseQuery(url.searchParams));
      res.writeHead(200);
      res.end(JSON.stringify(page));
      return;
    }

    res.writeHead(404);
    res.end(JSON.stringify({ error: "not found" }));
  } catch (err) {
    console.error(err);
    res.writeHead(500);
    res.end(JSON.stringify({ error: "internal error" }));
  }
});

/** Read event filters from the query string into a typed [`EventQuery`]. */
function parseQuery(params: URLSearchParams): EventQuery {
  const num = (v: string | null) => (v === null ? undefined : Number(v));
  return {
    contractId: params.get("contractId") ?? undefined,
    kind: params.get("kind") ?? undefined,
    fromLedger: num(params.get("fromLedger")),
    toLedger: num(params.get("toLedger")),
    limit: num(params.get("limit")),
    cursor: params.get("cursor") ?? undefined,
  };
}

server.listen(PORT, () => {
  console.log(`Stardex API listening on http://localhost:${PORT}`);
});
