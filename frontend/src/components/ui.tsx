import type { ReactNode } from "react";
import { Link } from "react-router-dom";

/** Pill badge used for eyebrow labels above headings. */
export function Badge({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-2 rounded-full border border-[var(--color-border)] bg-[var(--color-surface)]/60 px-3 py-1 text-xs font-medium text-slate-300">
      {children}
    </span>
  );
}

const primary =
  "inline-flex items-center justify-center gap-2 rounded-lg bg-[var(--color-accent)] px-5 py-2.5 text-sm font-semibold text-slate-950 transition hover:opacity-90";
const ghost =
  "inline-flex items-center justify-center gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)]/40 px-5 py-2.5 text-sm font-semibold text-slate-200 transition hover:bg-[var(--color-surface)]";

/** Internal primary call-to-action (router link). */
export function PrimaryLink({ to, children }: { to: string; children: ReactNode }) {
  return (
    <Link to={to} className={primary}>
      {children}
    </Link>
  );
}

/** External link styled as a button; `variant` picks the look. */
export function ExternalButton({
  href,
  children,
  variant = "ghost",
}: {
  href: string;
  children: ReactNode;
  variant?: "primary" | "ghost";
}) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className={variant === "primary" ? primary : ghost}
    >
      {children}
    </a>
  );
}

/** Centered eyebrow + title + subtitle block for section headers. */
export function SectionHeading({
  eyebrow,
  title,
  subtitle,
}: {
  eyebrow?: string;
  title: ReactNode;
  subtitle?: ReactNode;
}) {
  return (
    <div className="mx-auto max-w-2xl text-center">
      {eyebrow && (
        <p className="mb-3 text-xs font-semibold uppercase tracking-widest text-[var(--color-accent)]">
          {eyebrow}
        </p>
      )}
      <h2 className="text-2xl font-bold tracking-tight sm:text-3xl">{title}</h2>
      {subtitle && <p className="mt-3 text-sm text-slate-400 sm:text-base">{subtitle}</p>}
    </div>
  );
}
