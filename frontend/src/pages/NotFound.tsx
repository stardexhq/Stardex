import { PrimaryLink } from "../components/ui";

export function NotFound() {
  return (
    <div className="mx-auto flex max-w-6xl flex-col items-center px-6 py-32 text-center">
      <p className="font-mono text-6xl font-bold text-[var(--color-accent)]">404</p>
      <h1 className="mt-4 text-2xl font-bold tracking-tight">Page not found</h1>
      <p className="mt-2 max-w-md text-sm text-slate-400">
        That route isn&apos;t indexed. Head back home or jump straight to the live explorer.
      </p>
      <div className="mt-8">
        <PrimaryLink to="/">Back to home</PrimaryLink>
      </div>
    </div>
  );
}
