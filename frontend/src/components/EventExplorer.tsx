import { useState, type ReactNode } from "react";
import { useEvents, type EventFilters } from "../hooks/useEvents";
import { FilterBar } from "./FilterBar";
import { EventTable } from "./EventTable";

export function EventExplorer() {
  const [filters, setFilters] = useState<EventFilters>({});
  const {
    items,
    loading,
    refreshing,
    error,
    page,
    pageCount,
    hasNext,
    hasPrev,
    goToPage,
    next,
    prev,
    refresh,
  } = useEvents(filters.contractId, filters.kind);

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
        <p className="py-12 text-center text-sm text-slate-400">Loading events...</p>
      ) : items.length === 0 && !error ? (
        <p className="py-12 text-center text-sm text-slate-400">
          No events match these filters yet.
        </p>
      ) : (
        <>
          <EventTable events={items} />
          <Pagination
            page={page}
            pageCount={pageCount}
            hasNext={hasNext}
            hasPrev={hasPrev}
            loading={loading}
            count={items.length}
            onGoTo={goToPage}
            onNext={next}
            onPrev={prev}
          />
        </>
      )}
    </section>
  );
}

interface PaginationProps {
  page: number;
  pageCount: number;
  hasNext: boolean;
  hasPrev: boolean;
  loading: boolean;
  count: number;
  onGoTo: (page: number) => void;
  onNext: () => void;
  onPrev: () => void;
}

function Pagination({
  page,
  pageCount,
  hasNext,
  hasPrev,
  loading,
  count,
  onGoTo,
  onNext,
  onPrev,
}: PaginationProps) {
  const pages = Array.from({ length: pageCount }, (_, i) => i + 1);

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 text-xs text-slate-500">
      <span>
        Page {page} &middot; {count} event{count === 1 ? "" : "s"} on this page
      </span>
      <div className="flex items-center gap-1">
        <PageButton onClick={onPrev} disabled={!hasPrev || loading}>
          Prev
        </PageButton>
        {pages.map((n) => (
          <PageButton
            key={n}
            onClick={() => onGoTo(n)}
            disabled={loading}
            active={n === page}
          >
            {n}
          </PageButton>
        ))}
        <PageButton onClick={onNext} disabled={!hasNext || loading}>
          Next
        </PageButton>
      </div>
    </div>
  );
}

function PageButton({
  children,
  onClick,
  disabled,
  active,
}: {
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  active?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`min-w-[2.25rem] rounded-md border px-3 py-1.5 text-sm transition disabled:opacity-40 ${
        active
          ? "border-[var(--color-accent)] bg-[var(--color-accent)] text-slate-900"
          : "border-[var(--color-border)] text-slate-300 hover:bg-[var(--color-surface)]"
      }`}
    >
      {children}
    </button>
  );
}
