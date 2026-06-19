import { useState } from "react";
import { Link, NavLink } from "react-router-dom";
import { useHealth, type HealthStatus } from "../hooks/useHealth";
import { API_BASE_URL } from "../config";
import { GITHUB_URL, NAV_LINKS } from "../site";

const DOT: Record<HealthStatus, string> = {
  checking: "bg-amber-400",
  ok: "bg-emerald-400",
  down: "bg-amber-400",
  unavailable: "bg-slate-500",
};

const LABEL: Record<HealthStatus, string> = {
  checking: "Connecting...",
  ok: "API online",
  down: "API degraded",
  unavailable: "API status unavailable",
};

const TITLE: Record<HealthStatus, string> = {
  checking: API_BASE_URL,
  ok: API_BASE_URL,
  down: `${API_BASE_URL} responded but reports a problem`,
  unavailable: `Could not reach ${API_BASE_URL} (it may be offline, waking up, or blocked by a browser extension)`,
};

function HealthPill() {
  const status = useHealth();
  return (
    <span
      className="inline-flex items-center gap-2 rounded-full border border-[var(--color-border)] bg-[var(--color-surface)]/60 px-3 py-1 text-xs text-slate-400"
      title={TITLE[status]}
    >
      <span className={`h-2 w-2 rounded-full ${DOT[status]}`} />
      {LABEL[status]}
    </span>
  );
}

function linkClass({ isActive }: { isActive: boolean }) {
  return `rounded-md px-3 py-1.5 text-sm transition ${
    isActive
      ? "bg-[var(--color-surface)] text-white"
      : "text-slate-400 hover:text-white"
  }`;
}

export function NavBar() {
  const [open, setOpen] = useState(false);

  return (
    <header className="sticky top-0 z-40 border-b border-[var(--color-border)] bg-[var(--color-base)]/80 backdrop-blur">
      <div className="mx-auto flex max-w-6xl items-center justify-between px-6 py-3">
        <Link to="/" className="flex items-center gap-2.5" onClick={() => setOpen(false)}>
          <img src="/favicon.svg" alt="" className="h-7 w-7" />
          <span className="text-lg font-semibold tracking-tight">Stardex</span>
        </Link>

        <nav className="hidden items-center gap-1 md:flex">
          {NAV_LINKS.map((l) => (
            <NavLink key={l.to} to={l.to} end={l.end} className={linkClass}>
              {l.label}
            </NavLink>
          ))}
        </nav>

        <div className="hidden items-center gap-3 md:flex">
          <HealthPill />
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm text-slate-300 transition hover:bg-[var(--color-surface)]"
          >
            GitHub
          </a>
        </div>

        <button
          className="rounded-md border border-[var(--color-border)] px-3 py-1.5 text-sm text-slate-300 md:hidden"
          onClick={() => setOpen((v) => !v)}
          aria-label="Toggle navigation"
        >
          {open ? "Close" : "Menu"}
        </button>
      </div>

      {open && (
        <div className="border-t border-[var(--color-border)] px-6 py-3 md:hidden">
          <nav className="flex flex-col gap-1">
            {NAV_LINKS.map((l) => (
              <NavLink
                key={l.to}
                to={l.to}
                end={l.end}
                className={linkClass}
                onClick={() => setOpen(false)}
              >
                {l.label}
              </NavLink>
            ))}
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noreferrer"
              className="rounded-md px-3 py-1.5 text-sm text-slate-400 hover:text-white"
            >
              GitHub ↗
            </a>
            <div className="px-3 py-2">
              <HealthPill />
            </div>
          </nav>
        </div>
      )}
    </header>
  );
}
