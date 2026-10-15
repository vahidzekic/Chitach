// Global application state (card state, settings, navigation).
import { api } from "../api";
import type { AppInfo, AppState, ExportFormat, LicenseInfo, Mode, Settings, UpdateInfo } from "../api/types";
import { setLanguage, t, translateError } from "../i18n/index.svelte";
import { setTheme } from "../util/theme";
import { baseName, dirName, joinPath } from "../util/format";
import { toast } from "./toasts.svelte";

export type Page = "document" | "certificates" | "xmlSign" | "smartbox" | "settings" | "about";

interface Store {
  state: AppState;
  settings: Settings | null;
  /** mode the app was started with (mode changes apply after restart) */
  mode: Mode;
  info: AppInfo | null;
  page: Page;
  loaded: boolean;
  update: UpdateInfo | null;
  /** Chitach Pro licence; null until loaded */
  license: LicenseInfo | null;
}

export const app = $state<Store>({
  state: {
    status: "waiting",
    readers: [],
    selectedReader: null,
    document: null,
    atr: null,
    error: null,
    readId: 0,
  },
  settings: null,
  mode: "reader",
  info: null,
  page: "document",
  loaded: false,
  update: null,
  license: null,
});

export function homePage(mode: Mode): Page {
  return mode === "xmlSign" ? "xmlSign" : mode === "smartbox" ? "smartbox" : "document";
}

export function navigate(page: Page) {
  app.page = page;
}

export function showError(e: unknown, fallbackKey?: string) {
  const { title, detail } = translateError(e);
  if (fallbackKey && title === t("error.error")) {
    toast("error", t(fallbackKey), { detail });
  } else {
    toast("error", title, { detail });
  }
}

/** Apply look & language of the given settings without saving them. */
export function previewSettings(s: Settings) {
  setTheme(s.theme);
  setLanguage(s.language);
}

export async function saveSettings(s: Settings): Promise<boolean> {
  try {
    await api.saveSettings($state.snapshot(s) as Settings);
    app.settings = structuredClone($state.snapshot(s) as Settings);
    previewSettings(s);
    return true;
  } catch (e) {
    showError(e, "error.settings");
    return false;
  }
}

export async function checkForUpdate(silent: boolean): Promise<UpdateInfo | null> {
  try {
    const u = await api.checkUpdate();
    app.update = u;
    if (u.newer && silent) {
      toast("info", t("about.newVersionAvailable", u.latest), {
        action: { label: t("ui.download"), run: () => void api.openUrl(u.url) },
        timeout: 12000,
      });
    }
    return u;
  } catch (e) {
    if (!silent) showError(e, "error.updateCheck");
    return null;
  }
}

export async function init() {
  const params = new URLSearchParams(location.search);
  try {
    const [settings, state, info] = await Promise.all([api.getSettings(), api.getState(), api.getAppInfo()]);
    app.settings = settings;
    app.mode = settings.mode;
    app.state = state;
    app.info = info;
    previewSettings(settings);
    const p = params.get("page") as Page | null;
    app.page = p ?? homePage(settings.mode);
  } catch (e) {
    showError(e);
  }

  await api.onAppState((s) => {
    app.state = s;
  });
  await api.onNotify((n) => {
    const title = translateError(n.key).title;
    const action =
      n.level === "success" && n.detail && /[\\/]/.test(n.detail)
        ? { label: t("ui.open"), run: () => void openPath(n.detail!) }
        : undefined;
    const known = title !== t("error.error") || n.key === "error.error";
    toast(n.level, known ? title : n.key, {
      detail: n.detail ?? (known ? translateError(n.key).detail : undefined),
      action,
    });
  });
  void refreshLicense();
  app.loaded = true;

  if (app.settings?.checkUpdates && !api.isMock) void checkForUpdate(true);
}

export async function refreshLicense() {
  try {
    app.license = await api.licenseInfo();
  } catch {
    /* not critical: Pro features stay locked */
  }
}

/** Activate a Chitach Pro key; returns true on success. */
export async function activateLicense(key: string): Promise<boolean> {
  try {
    app.license = await api.licenseActivate(key);
    toast("success", t("pro.activated"));
    return true;
  } catch (e) {
    showError(e, "error.license");
    return false;
  }
}

export async function deactivateLicense(): Promise<void> {
  try {
    app.license = await api.licenseDeactivate();
    toast("success", t("pro.deactivated"));
  } catch (e) {
    showError(e, "error.license");
  }
}

export async function openPath(path: string) {
  try {
    await api.openPath(path);
  } catch (e) {
    showError(e, "error.openPath");
  }
}

const formatInfo: Record<ExportFormat, { name: string; ext: string; saved: string }> = {
  pdf: { name: "PDF", ext: "pdf", saved: "ui.pdfSaved" },
  json: { name: "JSON", ext: "json", saved: "ui.jsonSaved" },
  xlsx: { name: "Excel", ext: "xlsx", saved: "ui.xlsxSaved" },
};

/** Remember the last used directory locally (the backend persists it on export). */
function rememberDir(path: string) {
  const dir = dirName(path);
  if (app.settings && dir) app.settings.lastDir = dir;
}

/** Reload settings from the backend (it updates `lastDir` after export). */
async function refreshSettings() {
  try {
    app.settings = await api.getSettings();
  } catch {
    /* not critical */
  }
}

/** Ask for a path and export the current document. */
export async function exportDocument(format: ExportFormat): Promise<void> {
  const f = formatInfo[format];
  try {
    const name = await api.suggestFileName(format);
    const path = await api.saveDialog({
      defaultPath: joinPath(app.settings?.lastDir ?? "", name),
      filters: [{ name: f.name, extensions: [f.ext] }],
    });
    if (!path) return;
    const out = await api.exportDocument(format, path);
    rememberDir(out);
    void refreshSettings();
    toast("success", t(f.saved), {
      detail: baseName(out),
      action: { label: t("ui.open"), run: () => void openPath(out) },
    });
  } catch (e) {
    showError(e, "error.export");
  }
}

export async function saveFileAs(defaultName: string, ext: string, typeName: string): Promise<string | null> {
  const path = await api.saveDialog({
    defaultPath: joinPath(app.settings?.lastDir ?? "", defaultName),
    filters: [{ name: typeName, extensions: [ext] }],
  });
  if (path) rememberDir(path);
  return path;
}
