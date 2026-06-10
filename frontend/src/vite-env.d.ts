/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Base URL of the Stardex API (defaults to http://localhost:8080). */
  readonly VITE_STARDEX_API?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
