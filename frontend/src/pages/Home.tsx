import type { ReactNode } from "react";
import {
  Badge,
  ExternalButton,
  PrimaryLink,
  SectionHeading,
} from "../components/ui";
import { GITHUB_URL } from "../site";

export function Home() {
  return (
    <>
      <Hero />
      <Problem />
      <Features />
      <Pipeline />
      <CallToAction />
    </>
  );
}

function Hero() {
  return (
    <section className="relative overflow-hidden border-b border-[var(--color-border)] bg-grid">
      <div className="mx-auto max-w-6xl px-6 pb-20 pt-20 text-center sm:pt-28">
        <div className="mb-6 flex justify-center">
          <Badge>
            <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
            Open source · Apache-2.0 · Built on Stellar
          </Badge>
        </div>

        <h1 className="mx-auto max-w-4xl text-4xl font-bold leading-tight tracking-tight sm:text-6xl">
          The open-source indexer for{" "}
          <span className="text-gradient">Stellar &amp; Soroban</span>
        </h1>

        <p className="mx-auto mt-6 max-w-2xl text-base text-slate-400 sm:text-lg">
          Stellar&apos;s RPC only keeps a short window of history, then prunes it. Stardex
          captures every contract event durably and makes all of chain history queryable —
          through a REST API, a typed SDK, and this dashboard. Self-hostable, no exotic
          dependencies.
        </p>

        <div className="mt-9 flex flex-wrap items-center justify-center gap-3">
          <PrimaryLink to="/explorer">Explore live events →</PrimaryLink>
          <ExternalButton href={GITHUB_URL}>View on GitHub</ExternalButton>
        </div>

        <TerminalPreview />
      </div>
    </section>
  );
}

function TerminalPreview() {
  return (
    <div className="card-glow mx-auto mt-16 max-w-2xl overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]/70 text-left">
      <div className="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-2.5">
        <span className="h-3 w-3 rounded-full bg-rose-500/80" />
        <span className="h-3 w-3 rounded-full bg-amber-400/80" />
        <span className="h-3 w-3 rounded-full bg-emerald-400/80" />
        <span className="ml-2 text-xs text-slate-500">stardex — index</span>
      </div>
      <pre className="overflow-x-auto px-4 py-4 font-mono text-xs leading-relaxed text-slate-300">
        <span className="text-slate-500">$ </span>stardex index CDLZ…CYSC
        {"\n"}storage: Postgres (events + resumable cursor)
        {"\n"}indexing CDLZ… via soroban-testnet.stellar.org …
        {"\n\n"}
        <span className="text-[var(--color-accent)]">kind</span>     | fields
        {"\n"}---------+------------------------------------------------
        {"\n"}
        <span className="text-emerald-300">transfer</span> |{" "}
        {`{"from":"GBSO…","to":"GCML…","amount":"5000000"}`}
        {"\n"}
        <span className="text-slate-400">raw</span>      |{" "}
        {`{"topics":["AAAADwAAAANmZWUA", …],"data":"…"}`}
      </pre>
    </div>
  );
}

const QUESTIONS = [
  "Show me every payment this user has made over the last 6 months.",
  "What was this contract's volume, day by day?",
  "List all the streams, swaps, and mints this contract has ever emitted.",
];

function Problem() {
  return (
    <section className="mx-auto max-w-6xl px-6 py-20">
      <SectionHeading
        eyebrow="The problem"
        title="Chain history disappears — and everyone rebuilds the same plumbing"
        subtitle="Stellar's RPC is built for recent data. It keeps a short window, then records over its own footage. That makes the most common questions in any app surprisingly hard to answer:"
      />
      <div className="mx-auto mt-10 grid max-w-3xl gap-3">
        {QUESTIONS.map((q) => (
          <div
            key={q}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)]/40 px-5 py-4 text-sm text-slate-300"
          >
            <span className="mr-2 text-[var(--color-accent)]">“</span>
            {q}
          </div>
        ))}
      </div>
      <p className="mx-auto mt-8 max-w-2xl text-center text-sm text-slate-400">
        Today every Soroban team rebuilds the same indexing service from scratch. Stardex is
        that plumbing, built once — as a tool everyone can use and self-host.
      </p>
    </section>
  );
}

interface Feature {
  title: string;
  body: string;
  status: "live" | "soon";
}

const FEATURES: Feature[] = [
  {
    title: "Live event streaming",
    body: "Pages through a contract's events from Stellar RPC and polls for new ones, with retry/backoff through transient outages.",
    status: "live",
  },
  {
    title: "Resumable ingestion",
    body: "The cursor is persisted to Postgres, so a restart continues exactly where it left off — verified end-to-end on testnet.",
    status: "live",
  },
  {
    title: "Real decoders",
    body: "SAC / token transfers are decoded from raw XDR into typed { from, to, amount } records. New contracts = new decoders.",
    status: "live",
  },
  {
    title: "Durable Postgres store",
    body: "Every event is written to Postgres; events without a decoder yet are kept raw, so nothing is ever lost.",
    status: "live",
  },
  {
    title: "REST API",
    body: "GET /events serves indexed data with filters (contract, kind, ledger range) and cursor pagination; /health reports DB status.",
    status: "live",
  },
  {
    title: "Typed SDK & GraphQL",
    body: "A typed @stardex/sdk client and a GraphQL endpoint so apps consume history in a few lines. In active development.",
    status: "soon",
  },
];

function Features() {
  return (
    <section className="border-y border-[var(--color-border)] bg-[var(--color-surface)]/20">
      <div className="mx-auto max-w-6xl px-6 py-20">
        <SectionHeading
          eyebrow="What works today"
          title="A real engine, running against live testnet"
          subtitle="Stardex is in active development, but the core ingestion pipeline is real and battle-tested end-to-end."
        />
        <div className="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {FEATURES.map((f) => (
            <FeatureCard key={f.title} feature={f} />
          ))}
        </div>
      </div>
    </section>
  );
}

function FeatureCard({ feature }: { feature: Feature }) {
  return (
    <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]/50 p-5 transition hover:border-[var(--color-accent)]/40">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="font-semibold">{feature.title}</h3>
        <StatusTag status={feature.status} />
      </div>
      <p className="text-sm leading-relaxed text-slate-400">{feature.body}</p>
    </div>
  );
}

function StatusTag({ status }: { status: "live" | "soon" }) {
  if (status === "live") {
    return (
      <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-emerald-300">
        Live
      </span>
    );
  }
  return (
    <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-amber-300">
      Soon
    </span>
  );
}

const STAGES: { title: string; lang: string; lines: string[] }[] = [
  { title: "Ingestor", lang: "Rust", lines: ["RPC stream", "cursors", "retry / backfill"] },
  { title: "Decoders", lang: "Rust", lines: ["token / SAC", "swaps", "your own…"] },
  { title: "Postgres", lang: "SQL", lines: ["events", "cursors", "migrations"] },
  { title: "API", lang: "TypeScript", lines: ["REST + GraphQL", "SDK", "dashboard"] },
];

function Pipeline() {
  return (
    <section className="mx-auto max-w-6xl px-6 py-20">
      <SectionHeading
        eyebrow="Architecture"
        title="A neutral core, with pluggable decoders"
        subtitle="The ingestor knows how to read the chain; decoders know what each contract's events mean. Adding a contract is a new decoder — never a change to the engine."
      />
      <div className="mt-12 flex flex-col items-stretch gap-3 lg:flex-row lg:items-center">
        {STAGES.map((stage, i) => (
          <Stage key={stage.title} stage={stage} last={i === STAGES.length - 1} />
        ))}
      </div>
    </section>
  );
}

function Stage({
  stage,
  last,
}: {
  stage: { title: string; lang: string; lines: string[] };
  last: boolean;
}) {
  return (
    <>
      <div className="flex-1 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]/50 p-5">
        <div className="mb-2 flex items-center justify-between">
          <h3 className="font-semibold">{stage.title}</h3>
          <span className="font-mono text-[10px] uppercase tracking-wide text-slate-500">
            {stage.lang}
          </span>
        </div>
        <ul className="space-y-1 text-xs text-slate-400">
          {stage.lines.map((line) => (
            <li key={line}>• {line}</li>
          ))}
        </ul>
      </div>
      {!last && (
        <span className="self-center text-[var(--color-accent)] lg:rotate-0 rotate-90">→</span>
      )}
    </>
  );
}

function CallToAction() {
  return (
    <section className="mx-auto max-w-6xl px-6 pb-8">
      <Panel>
        <h2 className="text-2xl font-bold tracking-tight sm:text-3xl">
          Point Stardex at a contract and start capturing history
        </h2>
        <p className="mx-auto mt-3 max-w-xl text-sm text-slate-400">
          Self-hostable, open source, and built on plain Rust, Postgres, and TypeScript —
          clone it and run it.
        </p>
        <div className="mt-7 flex flex-wrap items-center justify-center gap-3">
          <PrimaryLink to="/explorer">Explore live events →</PrimaryLink>
          <ExternalButton href={GITHUB_URL} variant="primary">
            <span className="opacity-80">View on</span> GitHub ↗
          </ExternalButton>
        </div>
      </Panel>
    </section>
  );
}

function Panel({ children }: { children: ReactNode }) {
  return (
    <div className="card-glow relative overflow-hidden rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)]/60 px-6 py-14 text-center">
      <div className="pointer-events-none absolute inset-0 bg-grid opacity-40" />
      <div className="relative">{children}</div>
    </div>
  );
}
