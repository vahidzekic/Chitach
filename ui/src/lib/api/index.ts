// Chooses the Tauri implementation when running inside Tauri, otherwise the mock.
import type { Api } from "./types";
import { tauriApi } from "./tauri";
import { createMockApi } from "./mock";

export * from "./types";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: Api = inTauri ? tauriApi : createMockApi();
