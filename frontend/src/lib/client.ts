import { StardexClient } from "@stardex/sdk";
import { API_BASE_URL } from "../config";

/** Shared Stardex API client used across the dashboard. */
export const client = new StardexClient({ baseUrl: API_BASE_URL });
