/**
 * Stardex API server skeleton — the "front desk" that answers questions over
 * the web. See backlog issue #15.
 *
 * For now it's a dependency-free Node server with a /health check and a
 * /events endpoint that returns an empty page. TODO(#16): query Postgres
 * with filters + cursor pagination; TODO(#18): add GraphQL.
 */
import { createServer } from "node:http";
import type { Page, StardexEvent } from "@stardex/types";

const PORT = Number(process.env.PORT ?? 8080);

const server = createServer((req, res) => {
  const url = new URL(req.url ?? "/", `http://localhost:${PORT}`);

  if (url.pathname === "/health") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ status: "ok" }));
    return;
  }

  if (url.pathname === "/events") {
    // TODO(#16): read from Postgres with filters + cursor pagination.
    const page: Page<StardexEvent> = { items: [], nextCursor: null };
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(page));
    return;
  }

  res.writeHead(404, { "content-type": "application/json" });
  res.end(JSON.stringify({ error: "not found" }));
});

server.listen(PORT, () => {
  console.log(`Stardex API listening on http://localhost:${PORT}`);
});
