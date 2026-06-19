import { useEffect, useState } from "react";
import { API_BASE_URL } from "../config";

export type HealthStatus = "checking" | "ok" | "down" | "unavailable";

const TIMEOUT_MS = 8_000;

/**
 * One probe of the API `/health` endpoint. Never throws; maps the outcome to:
 * - `ok`          the API answered and is healthy
 * - `down`        the API answered but reports a problem (e.g. database)
 * - `unavailable` the request never reached the API (offline, timed out, or
 *                 blocked client-side by an extension / CORS)
 */
async function probe(): Promise<HealthStatus> {
  try {
    const res = await fetch(`${API_BASE_URL}/health`, {
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    return res.ok ? "ok" : "down";
  } catch {
    return "unavailable";
  }
}

/** Polls `/health` so the header can show connectivity. */
export function useHealth(intervalMs = 15_000): HealthStatus {
  const [status, setStatus] = useState<HealthStatus>("checking");

  useEffect(() => {
    let active = true;

    const check = async () => {
      // Retry a couple of times before settling on a bad status, so a cold
      // start (free hosting waking up) doesn't immediately read as a failure.
      for (let attempt = 0; attempt < 3; attempt++) {
        const result = await probe();
        if (!active) return;
        if (result === "ok" || attempt === 2) {
          setStatus(result);
          return;
        }
        await new Promise((r) => setTimeout(r, 2_000));
        if (!active) return;
      }
    };

    void check();
    const timer = setInterval(check, intervalMs);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [intervalMs]);

  return status;
}
