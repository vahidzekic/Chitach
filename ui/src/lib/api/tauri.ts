// Tauri implementation of the Api (see docs/04-api-ugovor.md).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  Api,
  AppInfo,
  AppState,
  CardVendor,
  CertInfo,
  Document,
  FinesCheckResult,
  FinesInfo,
  NotifyEvent,
  Settings,
  Slot,
  SmartboxStatus,
  TokenCert,
  UpdateInfo,
} from "./types";

/** Tauri rejects with whatever the command returned as error; normalise to string. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
  }
}

export const tauriApi: Api = {
  isMock: false,

  getAppInfo: () => call<AppInfo>("get_app_info"),
  getState: () => call<AppState>("get_state"),
  selectReader: (name) => call("select_reader", { name }),
  reloadCard: () => call("reload_card"),
  exportDocument: (format, path) => call<string>("export_document", { format, path }),
  suggestFileName: (format) => call<string>("suggest_file_name", { format }),
  updateRfzo: () => call<Document>("update_rfzo"),
  pinTriesLeft: () => call<number>("pin_tries_left"),
  changePin: (oldPin, newPin) => call("change_pin", { oldPin, newPin }),
  listCertificates: () => call<CertInfo[]>("list_certificates"),
  saveCertificate: (index, path) => call("save_certificate", { index, path }),
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings) => call("save_settings", { settings }),
  checkUpdate: () => call<UpdateInfo>("check_update"),
  openPath: (path) => call("open_path", { path }),
  openUrl: (url) => openUrl(url),
  signLoadModules: () => call<CardVendor[]>("sign_load_modules"),
  signListSlots: (vendor) => call<Slot[]>("sign_list_slots", { vendor }),
  signLogin: (vendor, slot, pin) => call<TokenCert[]>("sign_login", { vendor, slot, pin }),
  signXmlFile: (certId, inputPath, outputPath) => call("sign_xml_file", { certId, inputPath, outputPath }),
  signLogout: () => call("sign_logout"),
  smartboxStatus: () => call<SmartboxStatus>("smartbox_status"),
  finesProviders: () => call<FinesInfo>("fines_providers"),
  finesCheck: (providerId, force) => call<FinesCheckResult>("fines_check", { providerId, force: force ?? false }),

  onAppState: (cb) => listen<AppState>("app-state", (e) => cb(e.payload)),
  onNotify: (cb) => listen<NotifyEvent>("notify", (e) => cb(e.payload)),
  onSmartboxStatus: (cb) => listen<SmartboxStatus>("smartbox-status", (e) => cb(e.payload)),

  saveDialog: async (opts) => (await save(opts)) ?? null,
  openFileDialog: async (opts) => {
    const r = await open({ ...opts, multiple: false, directory: false });
    return typeof r === "string" ? r : null;
  },
  pickDirectory: async (opts) => {
    const r = await open({ ...opts, multiple: false, directory: true });
    return typeof r === "string" ? r : null;
  },
};
