import { useEffect, useState } from "react";
import { API_BASE_URL } from "../config";

export type HealthStatus = "checking" | "ok" | "down";

/** Polls the API `/health` endpoint so the header can show connectivity. */
export function useHealth(intervalMs = 15_000): HealthStatus {
  const [status, setStatus] = useState<HealthStatus>("checking");

  useEffect(() => {
    let active = true;

    const check = async () => {
      try {
        const res = await fetch(`${API_BASE_URL}/health`);
        if (active) setStatus(res.ok ? "ok" : "down");
      } catch {
        if (active) setStatus("down");
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
