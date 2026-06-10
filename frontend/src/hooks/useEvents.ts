import { useCallback, useEffect, useRef, useState } from "react";
import type { EventQuery, StardexEvent } from "@stardex/types";
import { client } from "../lib/client";
import { PAGE_SIZE } from "../config";

export interface EventFilters {
  contractId?: string;
  kind?: string;
}

export interface UseEventsResult {
  items: StardexEvent[];
  loading: boolean;
  error: string | null;
  hasMore: boolean;
  loadMore: () => void;
  refresh: () => void;
}

/**
 * Loads events from the Stardex API for the given filters, newest-first, with
 * "load more" cursor pagination. Filters are passed as primitives so the fetch
 * callback's dependencies stay stable.
 */
export function useEvents(contractId?: string, kind?: string): UseEventsResult {
  const [items, setItems] = useState<StardexEvent[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Identifies the latest in-flight request so stale responses are ignored
  // when filters change before an earlier fetch resolves.
  const requestId = useRef(0);

  const fetchPage = useCallback(
    async (cursor: string | undefined, append: boolean) => {
      const id = ++requestId.current;
      setLoading(true);
      setError(null);

      const query: EventQuery = { limit: PAGE_SIZE, cursor };
      if (contractId) query.contractId = contractId;
      if (kind) query.kind = kind;

      try {
        const page = await client.events(query);
        if (id !== requestId.current) return;
        setItems((prev) => (append ? [...prev, ...page.items] : page.items));
        setNextCursor(page.nextCursor);
      } catch (err) {
        if (id !== requestId.current) return;
        setError(err instanceof Error ? err.message : "Failed to load events");
      } finally {
        if (id === requestId.current) setLoading(false);
      }
    },
    [contractId, kind],
  );

  useEffect(() => {
    void fetchPage(undefined, false);
  }, [fetchPage]);

  const loadMore = useCallback(() => {
    if (nextCursor && !loading) void fetchPage(nextCursor, true);
  }, [nextCursor, loading, fetchPage]);

  const refresh = useCallback(() => {
    void fetchPage(undefined, false);
  }, [fetchPage]);

  return {
    items,
    loading,
    error,
    hasMore: nextCursor !== null,
    loadMore,
    refresh,
  };
}
