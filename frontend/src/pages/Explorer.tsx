import { EventExplorer } from "../components/EventExplorer";

export function Explorer() {
  return (
    <div className="mx-auto max-w-6xl px-6 py-12">
      <div className="mb-8">
        <p className="mb-2 text-xs font-semibold uppercase tracking-widest text-[var(--color-accent)]">
          Live explorer
        </p>
        <h1 className="text-2xl font-bold tracking-tight sm:text-3xl">Indexed events</h1>
        <p className="mt-2 max-w-2xl text-sm text-slate-400">
          Every contract event Stardex has indexed, newest first. Decoded where a decoder
          exists (e.g. <span className="font-mono text-slate-300">transfer</span>); everything
          else is kept <span className="font-mono text-slate-300">raw</span> so nothing is
          lost. Filter by contract or kind, and page through with “Load more.”
        </p>
      </div>
      <EventExplorer />
    </div>
  );
}
