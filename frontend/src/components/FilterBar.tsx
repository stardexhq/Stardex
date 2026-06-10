import { useState, type FormEvent } from "react";
import type { EventFilters } from "../hooks/useEvents";

interface FilterBarProps {
  filters: EventFilters;
  onApply: (filters: EventFilters) => void;
  onRefresh: () => void;
}

const inputClass =
  "rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-sm outline-none placeholder:text-slate-500 focus:border-[var(--color-accent)]";

export function FilterBar({ filters, onApply, onRefresh }: FilterBarProps) {
  const [contractId, setContractId] = useState(filters.contractId ?? "");
  const [kind, setKind] = useState(filters.kind ?? "");

  const submit = (e: FormEvent) => {
    e.preventDefault();
    onApply({
      contractId: contractId.trim() || undefined,
      kind: kind.trim() || undefined,
    });
  };

  const clear = () => {
    setContractId("");
    setKind("");
    onApply({});
  };

  const dirty = Boolean(filters.contractId || filters.kind);

  return (
    <form onSubmit={submit} className="flex flex-wrap items-center gap-2">
      <input
        className={`${inputClass} min-w-[18rem] flex-1`}
        placeholder="Contract ID (C…)"
        value={contractId}
        onChange={(e) => setContractId(e.target.value)}
        spellCheck={false}
      />
      <input
        className={`${inputClass} w-40`}
        placeholder="Kind (e.g. transfer)"
        value={kind}
        onChange={(e) => setKind(e.target.value)}
        spellCheck={false}
      />
      <button
        type="submit"
        className="rounded-md bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-slate-900 hover:opacity-90"
      >
        Apply
      </button>
      {dirty && (
        <button
          type="button"
          onClick={clear}
          className="rounded-md border border-[var(--color-border)] px-4 py-2 text-sm text-slate-300 hover:bg-[var(--color-surface)]"
        >
          Clear
        </button>
      )}
      <button
        type="button"
        onClick={onRefresh}
        className="rounded-md border border-[var(--color-border)] px-4 py-2 text-sm text-slate-300 hover:bg-[var(--color-surface)]"
      >
        Refresh
      </button>
    </form>
  );
}
