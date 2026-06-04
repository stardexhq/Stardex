/**
 * Typed client for the Stardex API — the "easy button" other apps use to
 * query indexed data. See backlog issue #21.
 */
import type { EventQuery, Page, StardexEvent } from "@stardex/types";

export interface StardexClientOptions {
  /** Base URL of a running Stardex API, e.g. "http://localhost:8080". */
  baseUrl: string;
  /** Optional custom fetch (defaults to the global fetch). */
  fetch?: typeof fetch;
}

export class StardexClient {
  private readonly baseUrl: string;
  private readonly fetchFn: typeof fetch;

  constructor(opts: StardexClientOptions) {
    this.baseUrl = opts.baseUrl.replace(/\/$/, "");
    this.fetchFn = opts.fetch ?? fetch;
  }

  /** Query indexed events with optional filters + cursor pagination. */
  async events(query: EventQuery = {}): Promise<Page<StardexEvent>> {
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(query)) {
      if (value !== undefined) params.set(key, String(value));
    }
    const res = await this.fetchFn(`${this.baseUrl}/events?${params}`);
    if (!res.ok) throw new Error(`Stardex API error ${res.status}`);
    return (await res.json()) as Page<StardexEvent>;
  }
}
