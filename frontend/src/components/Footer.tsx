import { Link } from "react-router-dom";
import { GITHUB_URL, STELLAR_URL, NAV_LINKS } from "../site";

export function Footer() {
  return (
    <footer className="mt-24 border-t border-[var(--color-border)]">
      <div className="mx-auto flex max-w-6xl flex-col gap-6 px-6 py-10 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-2.5">
          <img src="/favicon.svg" alt="" className="h-6 w-6" />
          <div>
            <p className="text-sm font-semibold">Stardex</p>
            <p className="text-xs text-slate-500">
              Open-source indexer for Stellar / Soroban · Apache-2.0
            </p>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-sm text-slate-400">
          {NAV_LINKS.map((l) => (
            <Link key={l.to} to={l.to} className="hover:text-white">
              {l.label}
            </Link>
          ))}
          <a href={GITHUB_URL} target="_blank" rel="noreferrer" className="hover:text-white">
            GitHub ↗
          </a>
          <a href={STELLAR_URL} target="_blank" rel="noreferrer" className="hover:text-white">
            Stellar ↗
          </a>
        </div>
      </div>
    </footer>
  );
}
