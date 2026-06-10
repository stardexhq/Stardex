import { useHealth } from "../hooks/useHealth";
import { API_BASE_URL } from "../config";

const DOT: Record<string, string> = {
  checking: "bg-amber-400",
  ok: "bg-emerald-400",
  down: "bg-rose-500",
};

const LABEL: Record<string, string> = {
  checking: "Connecting…",
  ok: "API online",
  down: "API offline",
};

export function Header() {
  const status = useHealth();

  return (
    <header className="border-b border-[var(--color-border)]">
      <div className="mx-auto flex max-w-6xl items-center justify-between px-6 py-4">
        <div className="flex items-center gap-3">
          <img src="/favicon.svg" alt="" className="h-7 w-7" />
          <div>
            <h1 className="text-lg font-semibold tracking-tight">Stardex</h1>
            <p className="text-xs text-slate-400">Stellar / Soroban event explorer</p>
          </div>
        </div>

        <div
          className="flex items-center gap-2 text-xs text-slate-400"
          title={API_BASE_URL}
        >
          <span className={`h-2 w-2 rounded-full ${DOT[status]}`} />
          {LABEL[status]}
        </div>
      </div>
    </header>
  );
}
