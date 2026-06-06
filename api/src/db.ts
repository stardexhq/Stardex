/**
 * Postgres connection for the API. A single pooled client shared by all
 * requests; configured via DATABASE_URL with a local-dev default.
 */
import { Pool } from "pg";

const connectionString =
  process.env.DATABASE_URL ?? "postgres://stardex:stardex@localhost:5432/stardex";

export const pool = new Pool({ connectionString, max: 10 });

/** True if the database answers a trivial query — used by /health. */
export async function pingDb(): Promise<boolean> {
  try {
    await pool.query("select 1");
    return true;
  } catch {
    return false;
  }
}
