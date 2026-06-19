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
  refreshing: boolean;
  error: string | null;
  /** Current page, 1-based. Page 1 is always the most recent events. */
  page: number;
  /** Pages discovered so far (the current page plus any known next page). */
  pageCount: number;
  hasNext: boolean;
  hasPrev: boolean;
  goToPage: (page: number) => void;
  next: () => void;
  prev: () => void;
  refresh: () => void;
}

/**
 * Loads one page (50) of events at a time, newest-first. Page 1 is the most
 * recent events; navigating forward walks back through history via the API's
 * opaque cursor. Cursors are remembered per page so Prev/Next and numbered
 * jumps work without re-fetching from the start.
 */
export function useEvents(contractId?: string, kind?: string): UseEventsResult {
  const [items, setItems] = useState<StardexEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Cursor to fetch each page index (0-based). cursors[0] is always undefined
  // (the first page); cursors[i] is filled in once page i-1 has been loaded.
  const [cursors, setCursors] = useState<(string | undefined)[]>([undefined]);
  const [pageIndex, setPageIndex] = useState(0);
  const [hasNext, setHasNext] = useState(false);

  // Identifies the latest in-flight request so stale responses are ignored.
  const requestId = useRef(0);

  const fetchPage = useCallback(
    async (index: number, cursor: string | undefined) => {
      const id = ++requestId.current;
      setLoading(true);
      setError(null);

      const query: EventQuery = { limit: PAGE_SIZE, cursor };
      if (contractId) query.contractId = contractId;
      if (kind) query.kind = kind;

      try {
        const result = await client.events(query);
        if (id !== requestId.current) return;
        setItems(result.items);
        setPageIndex(index);
        setHasNext(result.nextCursor !== null);
        // Remember the cursor for the next page (or trim if this is the last).
        setCursors((prev) => {
          const copy = prev.slice(0, index + 1);
          if (result.nextCursor) copy[index + 1] = result.nextCursor;
          return copy;
        });
      } catch (err) {
        if (id !== requestId.current) return;
        setError(err instanceof Error ? err.message : "Failed to load events");
      } finally {
        if (id === requestId.current) setLoading(false);
      }
    },
    [contractId, kind],
  );

  // Reset to the most recent page whenever the filters change.
  useEffect(() => {
    setCursors([undefined]);
    void fetchPage(0, undefined);
  }, [fetchPage]);

  const goToPage = useCallback(
    (page: number) => {
      const index = page - 1;
      if (loading || index < 0 || index >= cursors.length) return;
      void fetchPage(index, cursors[index]);
    },
    [loading, cursors, fetchPage],
  );

  const next = useCallback(() => {
    if (loading || !hasNext) return;
    const index = pageIndex + 1;
    void fetchPage(index, cursors[index]);
  }, [loading, hasNext, pageIndex, cursors, fetchPage]);

  const prev = useCallback(() => {
    if (loading || pageIndex === 0) return;
    const index = pageIndex - 1;
    void fetchPage(index, cursors[index]);
  }, [loading, pageIndex, cursors, fetchPage]);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      setCursors([undefined]);
      // Keep the indicator visible briefly so a fast response still registers.
      await Promise.all([
        fetchPage(0, undefined),
        new Promise((r) => setTimeout(r, 400)),
      ]);
    } finally {
      setRefreshing(false);
    }
  }, [fetchPage]);

  return {
    items,
    loading,
    refreshing,
    error,
    page: pageIndex + 1,
    pageCount: cursors.length,
    hasNext,
    hasPrev: pageIndex > 0,
    goToPage,
    next,
    prev,
    refresh,
  };
}
