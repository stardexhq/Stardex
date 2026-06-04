/**
 * Shared types for Stardex — the "dictionary" the API and SDK both speak.
 * See backlog issue #20.
 */

/** A decoded contract event, as stored and returned by the API. */
export interface StardexEvent {
  /** Auto-incrementing id assigned by the store. */
  id: string;
  /** Contract that emitted the event. */
  contractId: string;
  /** Ledger (block) sequence the event was emitted in. */
  ledger: number;
  /** Decoder-defined event kind, e.g. "transfer", "swap". */
  kind: string;
  /** Decoded key/value fields. */
  fields: Record<string, string>;
  /** ISO timestamp of the ledger close. */
  closedAt: string;
}

/** A cursor-paginated list response. */
export interface Page<T> {
  items: T[];
  /** Opaque cursor for the next page, or null at the end. */
  nextCursor: string | null;
}

/** Filters accepted by event queries. */
export interface EventQuery {
  contractId?: string;
  kind?: string;
  fromLedger?: number;
  toLedger?: number;
  limit?: number;
  cursor?: string;
}
