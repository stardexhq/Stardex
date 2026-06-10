import { Header } from "./components/Header";
import { EventExplorer } from "./components/EventExplorer";

export default function App() {
  return (
    <div className="min-h-screen">
      <Header />
      <main className="mx-auto max-w-6xl px-6 py-8">
        <div className="mb-6">
          <h2 className="text-xl font-semibold tracking-tight">Events</h2>
          <p className="text-sm text-slate-400">
            Every decoded contract event Stardex has indexed, newest first.
          </p>
        </div>
        <EventExplorer />
      </main>
    </div>
  );
}
