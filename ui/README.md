# Chitach UI

Svelte 5 + TypeScript + Vite frontend for the Chitach Tauri app.

- `npm run dev` – runs in the browser with a **mock backend** (sample documents, fake readers).
  URL parameters: `?mock=demo|waiting|id|medical|vehicle|reading|noReader|unknown|error`,
  `&mode=reader|xmlSign|smartbox`, `&theme=light|dark|system`, `&lang=sr-Latn|sr-Cyrl|en|ru`,
  `&page=document|certificates|xmlSign|smartbox|settings|about`.
  `window.__chitachMock.insert("medical")` / `.remove()` simulate card insertion.
- `npm run check` – svelte-check + tsc.
- `npm run build` – production build into `dist/` (Tauri `frontendDist`).

The Rust ↔ UI contract is documented in `docs/04-api-ugovor.md`; types live in `src/lib/api/types.ts`.
Translations (`src/lib/i18n/*.json`) are derived from Bas Celik (MIT) plus Chitach keys.
