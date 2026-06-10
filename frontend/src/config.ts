/** Base URL of the Stardex API the dashboard reads from. */
export const API_BASE_URL =
  import.meta.env.VITE_STARDEX_API ?? "http://localhost:8080";

/** Rows fetched per page in the event explorer. */
export const PAGE_SIZE = 50;
