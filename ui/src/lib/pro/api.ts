// Chitach Pro commands (see docs/04-api-ugovor.md): Tauri implementation and the browser
// mock. `api` is the app's Api plus these, for the Pro components.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { api as baseApi, type Api } from "../api";
import { sampleCerts, sampleTokenCerts } from "./mockData";
import { mockFinesCheck, mockFinesInfo } from "./mockFines";
import { mockHistoryApiCheck, mockHistoryInfo, mockHistoryRemoveKey, mockHistorySetKey } from "./mockHistory";
import type {
  CardVendor,
  CertInfo,
  FinesCheckResult,
  FinesInfo,
  HistoryApiResult,
  HistoryInfo,
  LicenseInfo,
  Slot,
  SmartboxStatus,
  TokenCert,
  Unlisten,
} from "./types";

export interface ProApi {
  pinTriesLeft(): Promise<number>;
  changePin(oldPin: string, newPin: string): Promise<void>;
  listCertificates(): Promise<CertInfo[]>;
  saveCertificate(index: number, path: string): Promise<void>;
  signLoadModules(): Promise<CardVendor[]>;
  signListSlots(vendor: CardVendor): Promise<Slot[]>;
  signLogin(vendor: CardVendor, slot: number, pin: string): Promise<TokenCert[]>;
  signXmlFile(certId: string, inputPath: string, outputPath: string): Promise<void>;
  signLogout(): Promise<void>;
  smartboxStatus(): Promise<SmartboxStatus>;
  finesProviders(): Promise<FinesInfo>;
  finesCheck(providerId: string, force?: boolean): Promise<FinesCheckResult>;
  historyInfo(): Promise<HistoryInfo>;
  historySetApiKey(providerId: string, apiKey: string, secret: string): Promise<void>;
  historyRemoveApiKey(providerId: string): Promise<void>;
  historyApiCheck(providerId: string, operation: string, force?: boolean): Promise<HistoryApiResult>;
  licenseInfo(): Promise<LicenseInfo>;
  licenseActivate(key: string): Promise<LicenseInfo>;
  licenseDeactivate(): Promise<LicenseInfo>;
  onSmartboxStatus(cb: (s: SmartboxStatus) => void): Promise<Unlisten>;
}

/** Tauri rejects with whatever the command returned as error; normalise to string. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
  }
}

const tauriProApi: ProApi = {
  pinTriesLeft: () => call<number>("pin_tries_left"),
  changePin: (oldPin, newPin) => call("change_pin", { oldPin, newPin }),
  listCertificates: () => call<CertInfo[]>("list_certificates"),
  saveCertificate: (index, path) => call("save_certificate", { index, path }),
  signLoadModules: () => call<CardVendor[]>("sign_load_modules"),
  signListSlots: (vendor) => call<Slot[]>("sign_list_slots", { vendor }),
  signLogin: (vendor, slot, pin) => call<TokenCert[]>("sign_login", { vendor, slot, pin }),
  signXmlFile: (certId, inputPath, outputPath) => call("sign_xml_file", { certId, inputPath, outputPath }),
  signLogout: () => call("sign_logout"),
  smartboxStatus: () => call<SmartboxStatus>("smartbox_status"),
  finesProviders: () => call<FinesInfo>("fines_providers"),
  finesCheck: (providerId, force) => call<FinesCheckResult>("fines_check", { providerId, force: force ?? false }),
  historyInfo: () => call<HistoryInfo>("history_info"),
  historySetApiKey: (providerId, apiKey, secret) => call("history_set_api_key", { providerId, apiKey, secret }),
  historyRemoveApiKey: (providerId) => call("history_remove_api_key", { providerId }),
  historyApiCheck: (providerId, operation, force) =>
    call<HistoryApiResult>("history_api_check", { providerId, operation, force: force ?? false }),
  licenseInfo: () => call<LicenseInfo>("license_info"),
  licenseActivate: (key) => call<LicenseInfo>("license_activate", { key }),
  licenseDeactivate: () => call<LicenseInfo>("license_deactivate"),
  onSmartboxStatus: (cb) => listen<SmartboxStatus>("smartbox-status", (e) => cb(e.payload)),
};

const LICENSE_KEY = "chitach.mock.license";
const MOCK_PRO_KEY = "CHITACH-PRO-DEMO";

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

/** Browser mock (`npm run dev`); reads the card and settings from the app's mock Api.
 * URL parameters: `?pro=0|1` (gating on: 0 = locked, activate with CHITACH-PRO-DEMO;
 * 1 = activated), `&fines=…`, `&history=…`, `&historyKey=1` (see `../api/mock.ts`). */
function createMockProApi(base: Api): ProApi {
  const params = new URLSearchParams(location.search);
  // Pro gating is off unless `?pro=` is given (as in a build without a configured store).
  const proEnabled = params.has("pro");
  let pro = params.get("pro") !== "0";
  try {
    const raw = localStorage.getItem(LICENSE_KEY);
    if (raw !== null && params.get("pro") === null) pro = raw === "1";
  } catch {
    /* storage unavailable */
  }
  const features: LicenseInfo["features"] = ["dataExport", "autoSave", "background", "pin", "certificates", "xmlSign", "smartbox", "fines", "history"];
  const licenseInfo = (): LicenseInfo =>
    pro
      ? { enabled: proEnabled, active: true, stored: true, keyHint: "…DEMO", email: "kupac@example.com", validatedAt: Date.now(), buyUrl: null, features }
      : { enabled: proEnabled, active: false, stored: false, keyHint: null, email: null, validatedAt: null, buyUrl: "https://example.com/chitach-pro", features };
  const setPro = (on: boolean) => {
    pro = on;
    try {
      localStorage.setItem(LICENSE_KEY, on ? "1" : "0");
    } catch {
      /* storage unavailable */
    }
  };
  if (params.get("historyKey") === "1") void mockHistorySetKey("vincario", "demo-key", "demo-secret");

  const doc = async () => (await base.getState()).document;
  const smartbox: SmartboxStatus = {
    running: true,
    address: "127.0.0.1:20090",
    vendors: ["mup", "halcom"],
    error: null,
  };
  const smartboxListeners = new Set<(s: SmartboxStatus) => void>();
  let loggedIn = false;

  return {
    async pinTriesLeft() {
      if ((await doc())?.kind !== "id") throw "error.notIdCard";
      return 3;
    },
    async changePin(oldPin) {
      await delay(700);
      if (oldPin === "0000") throw "error.wrongPin:2";
    },
    async listCertificates() {
      await delay(300);
      if ((await doc())?.kind !== "id") throw "error.notIdCard";
      return clone(sampleCerts);
    },
    async saveCertificate() {
      await delay(200);
    },
    async signLoadModules() {
      await delay(300);
      return ["mup", "halcom"] as CardVendor[];
    },
    async signListSlots(vendor) {
      await delay(300);
      return vendor === "mup"
        ? [{ id: 0, description: "ACS ACR39U ICC Reader 00 00 (MUP eID)" }]
        : [{ id: 1, description: "Halcom token (eToken 5110)" }];
    },
    async signLogin(_vendor, _slot, pin) {
      await delay(700);
      if (pin === "0000") throw "error.wrongPin";
      loggedIn = true;
      return clone(sampleTokenCerts);
    },
    async signXmlFile(_certId, _inputPath, outputPath) {
      await delay(900);
      if (!loggedIn) throw "error.notLoggedIn";
      console.info("[mock] signed ->", outputPath);
    },
    async signLogout() {
      loggedIn = false;
    },
    async smartboxStatus() {
      return clone(smartbox);
    },
    async finesProviders() {
      const d = await doc();
      return mockFinesInfo(d?.kind === "vehicle" ? d.data.RegistrationNumberOfVehicle : "");
    },
    async finesCheck(providerId, force = false) {
      if (proEnabled && !pro) throw "error.proRequired";
      const d = await doc();
      if (d?.kind !== "vehicle") throw "error.notVehicleCard";
      return mockFinesCheck(d.data.RegistrationNumberOfVehicle, providerId, force, await base.getSettings());
    },
    async historyInfo() {
      const d = await doc();
      return mockHistoryInfo(d?.kind === "vehicle" ? d.data : null);
    },
    async historySetApiKey(providerId, apiKey, secret) {
      await mockHistorySetKey(providerId, apiKey, secret);
    },
    async historyRemoveApiKey(providerId) {
      await mockHistoryRemoveKey(providerId);
    },
    async historyApiCheck(providerId, operation, force = false) {
      if (proEnabled && !pro) throw "error.proRequired";
      const d = await doc();
      if (d?.kind !== "vehicle") throw "error.notVehicleCard";
      return mockHistoryApiCheck(d.data, providerId, operation, force, await base.getSettings());
    },
    async licenseInfo() {
      return licenseInfo();
    },
    async licenseActivate(key) {
      await delay(600);
      if (!key.trim()) throw "error.licenseNoKey";
      if (key.trim() !== MOCK_PRO_KEY) throw "error.licenseInvalid|license_key not found.";
      setPro(true);
      return licenseInfo();
    },
    async licenseDeactivate() {
      await delay(300);
      setPro(false);
      return licenseInfo();
    },
    async onSmartboxStatus(cb) {
      smartboxListeners.add(cb);
      return () => smartboxListeners.delete(cb);
    },
  };
}

const proApi: ProApi = baseApi.isMock ? createMockProApi(baseApi) : tauriProApi;

/** The app's Api plus the Pro commands. */
export const api: Api & ProApi = { ...baseApi, ...proApi };
