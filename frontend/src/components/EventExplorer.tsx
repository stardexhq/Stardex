import { useState } from "react";
import { useEvents, type EventFilters } from "../hooks/useEvents";
import { FilterBar } from "./FilterBar";
import { EventTable } from "./EventTable";

export function EventExplorer() {
  const [filters, setFilters] = useState<EventFilters>({});
  const { items, loading, refreshing, error, hasMore, loadMore, refresh } =
    useEvents(filters.contractId, filters.kind);

  const initialLoading = loading && items.length === 0;

  return (
    <section className="flex flex-col gap-4">
      <FilterBar
        filters={filters}
        onApply={setFilters}
        onRefresh={refresh}
        refreshing={refreshing}
      />

      {error && (
        <div className="rounded-lg border border-rose-500/40 bg-rose-500/10 px-4 py-3 text-sm text-rose-200">
          {error}. Is the API running at the configured URL?
        </div>
      )}

      {initialLoading ? (
        <p className="py-12 text-center text-sm text-slate-400">Loading events…</p>
      ) : items.length === 0 && !error ? (
        <p className="py-12 text-center text-sm text-slate-400">
          No events match these filters yet.
        </p>
      ) : (
        <>
          <EventTable events={items} />
          <div className="flex items-center justify-between text-xs text-slate-500">
            <span>
              {items.length} event{items.length === 1 ? "" : "s"} loaded
            </span>
            {hasMore && (
              <button
                onClick={loadMore}
                disabled={loading}
                className="rounded-md border border-[var(--color-border)] px-4 py-2 text-sm text-slate-300 hover:bg-[var(--color-surface)] disabled:opacity-50"
              >
                {loading ? "Loading…" : "Load more"}
              </button>
            )}
          </div>
        </>
      )}
    </section>
  );
}
