// Mock implementation of the Api used when the UI runs outside Tauri
// (`npm run dev` in a browser), for development and screenshots.
//
// URL parameters (all optional):
//   ?mock=demo|waiting|id|medical|vehicle|reading|noReader|unknown|error
//   &mode=reader|xmlSign|smartbox  &theme=light|dark|system  &lang=sr-Latn|sr-Cyrl|en|ru
//   &page=document|certificates|xmlSign|smartbox|settings|about
//   &fines=found|clean|manual|unavailable|parseError  (forced automatic fines check result)
//   &history=ok|unauthorized|error|unavailable|parseError  (forced vehicle history API result)
//   &historyKey=1  (a vehicle history API key is already stored)
// A small control object is exposed as `window.__chitachMock` (insert/remove cards).
import type {
  Api,
  AppState,
  CardVendor,
  Document,
  DocumentKind,
  NotifyEvent,
  ReaderStatus,
  Settings,
  SmartboxStatus,
} from "./types";
import { mockFinesCheck, mockFinesInfo } from "./mockFines";
import { mockHistoryApiCheck, mockHistoryInfo, mockHistoryRemoveKey, mockHistorySetKey } from "./mockHistory";
import { sampleCerts, sampleId, sampleMedical, sampleTokenCerts, sampleVehicle } from "./mockData";

const SETTINGS_KEY = "chitach.mock.settings";

export const defaultSettings: Settings = {
  language: "sr-Latn",
  theme: "system",
  mode: "reader",
  pdfCyrillicLabels: false,
  autoSave: "off",
  autoSaveDir: "",
  runInBackground: false,
  checkUpdates: true,
  lastDir: "",
  pkcs11Paths: { mup: "", pks: "", posta: "", halcom: "", esmart: "" },
  fines: { autoCheck: false, consentGiven: false },
  history: { consentGiven: false },
};

interface MockReader {
  name: string;
  card: DocumentKind | "unknown" | null;
}

const ATRS: Record<string, string> = {
  id: "3B FF 94 00 00 81 31 80 43 80 31 80 65 B0 85 02 01 F3 12 0F FF 82 90 00 79",
  medical: "3B F4 13 00 00 81 31 FE 45 52 46 5A 4F ED",
  vehicle: "3B DB 96 00 80 B1 FE 45 1F 83 00 31 C0 64 1A 18 01 00 07 90 00 5A",
  unknown: "3B 8F 80 01 80 4F 0C A0 00 00 03 06 03 00 03 00 00 00 00 68",
};

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;

function makeDoc(kind: DocumentKind): Document {
  switch (kind) {
    case "id":
      return { kind, data: sampleId() };
    case "medical":
      return { kind, data: sampleMedical() };
    case "vehicle":
      return { kind, data: sampleVehicle() };
  }
}

function baseName(p: string): string {
  return p.split(/[\\/]/).pop() ?? p;
}

export function createMockApi(): Api {
  const params = new URLSearchParams(location.search);
  const scenario = params.get("mock") ?? "demo";

  let settings: Settings = clone(defaultSettings);
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (raw) settings = { ...settings, ...(JSON.parse(raw) as Partial<Settings>) };
  } catch {
    /* storage unavailable */
  }
  const mode = params.get("mode");
  if (mode === "reader" || mode === "xmlSign" || mode === "smartbox") settings.mode = mode;
  const theme = params.get("theme");
  if (theme === "light" || theme === "dark" || theme === "system") settings.theme = theme;
  const lang = params.get("lang");
  if (lang === "sr-Latn" || lang === "sr-Cyrl" || lang === "en" || lang === "ru") settings.language = lang;
  if (params.get("historyKey") === "1") void mockHistorySetKey("vincario", "demo-key", "demo-secret");

  const readers: MockReader[] = [
    { name: "ACS ACR39U ICC Reader 00 00", card: null },
    { name: "Gemalto PC Twin Reader 01 00", card: null },
    { name: "HID Global OMNIKEY 3x21 Smart Card Reader 02 00", card: null },
  ];

  const state: AppState = {
    status: "waiting",
    readers: [],
    selectedReader: null,
    document: null,
    atr: null,
    error: null,
    readId: 0,
  };

  const stateListeners = new Set<(s: AppState) => void>();
  const notifyListeners = new Set<(n: NotifyEvent) => void>();
  const smartboxListeners = new Set<(s: SmartboxStatus) => void>();

  function readerStatuses(): ReaderStatus[] {
    return readers.map((r) => ({ name: r.name, cardPresent: r.card !== null, atr: r.card ? ATRS[r.card] : null }));
  }

  function emit() {
    state.readers = readerStatuses();
    const snap = clone(state);
    stateListeners.forEach((cb) => cb(snap));
  }

  function activeReader(): MockReader | undefined {
    if (state.selectedReader) return readers.find((r) => r.name === state.selectedReader);
    return readers.find((r) => r.card !== null) ?? readers[0];
  }

  let readToken = 0;
  async function readActive(instant = false) {
    const token = ++readToken;
    const r = activeReader();
    if (readers.length === 0 || !r) {
      Object.assign(state, { status: "noReader", document: null, atr: null, error: null });
      emit();
      return;
    }
    if (!r.card) {
      Object.assign(state, { status: "waiting", document: null, atr: null, error: null });
      emit();
      return;
    }
    if (r.card === "unknown") {
      Object.assign(state, { status: "unknownCard", document: null, atr: ATRS.unknown, error: null });
      emit();
      return;
    }
    if (!instant) {
      Object.assign(state, { status: "reading", document: null, atr: ATRS[r.card], error: null });
      emit();
      await delay(1300);
      if (token !== readToken) return;
    }
    Object.assign(state, {
      status: "ready",
      document: makeDoc(r.card),
      atr: ATRS[r.card],
      error: null,
      readId: state.readId + 1,
    });
    emit();
    if (settings.autoSave !== "off") {
      notifyListeners.forEach((cb) => cb({ level: "success", key: "autoSave.done" }));
    }
  }

  // Initial scenario.
  switch (scenario) {
    case "noReader":
      readers.length = 0;
      state.status = "noReader";
      break;
    case "id":
    case "medical":
    case "vehicle": {
      const idx = { id: 0, medical: 1, vehicle: 2 }[scenario];
      readers[idx].card = scenario;
      state.selectedReader = readers[idx].name;
      state.status = "ready";
      state.document = makeDoc(scenario);
      state.atr = ATRS[scenario];
      state.readId = 1;
      break;
    }
    case "reading":
      readers[0].card = "id";
      state.status = "reading";
      state.atr = ATRS.id;
      break;
    case "unknown":
      readers[0].card = "unknown";
      state.status = "unknownCard";
      state.atr = ATRS.unknown;
      break;
    case "error":
      readers[0].card = "id";
      state.status = "error";
      state.error = "error.readingCard|SCARD_W_UNPOWERED_CARD (0x80100067)";
      break;
    case "waiting":
      break;
    default:
      // demo: insert an ID card after a short while
      setTimeout(() => {
        readers[0].card = "id";
        void readActive();
      }, 2500);
  }
  state.readers = readerStatuses();

  const control = {
    insert(kind: DocumentKind | "unknown" = "id", reader = 0) {
      readers[reader].card = kind;
      void readActive();
    },
    remove(reader = 0) {
      readers[reader].card = null;
      void readActive();
    },
    error(msg = "error.readingCard|SCARD_E_NO_SMARTCARD") {
      Object.assign(state, { status: "error", document: null, error: msg });
      emit();
    },
    notify(n: NotifyEvent) {
      notifyListeners.forEach((cb) => cb(n));
    },
  };
  (window as unknown as { __chitachMock: typeof control }).__chitachMock = control;

  const smartbox: SmartboxStatus = {
    running: true,
    address: "127.0.0.1:20090",
    vendors: ["mup", "halcom"],
    error: null,
  };
  let loggedIn = false;

  const api: Api = {
    isMock: true,

    async getAppInfo() {
      return { version: "0.1.0", os: "linux", arch: "x86_64" };
    },
    async getState() {
      state.readers = readerStatuses();
      return clone(state);
    },
    async selectReader(name) {
      state.selectedReader = name;
      await readActive();
    },
    async reloadCard() {
      await readActive();
    },
    async exportDocument(_format, path) {
      await delay(400);
      settings.lastDir = path.substring(0, Math.max(path.lastIndexOf("/"), 0));
      return path;
    },
    async suggestFileName(format) {
      const d = state.document;
      let stem = "dokument";
      if (d?.kind === "id") stem = `${d.data.GivenName}_${d.data.Surname}`;
      if (d?.kind === "medical") stem = `${d.data.GivenNameLatin}_${d.data.FamilyNameLatin}`;
      if (d?.kind === "vehicle")
        stem = `${d.data.RegistrationNumberOfVehicle}_${d.data.OwnersSurnameOrBusinessName}_${d.data.OwnerName}`;
      return `${stem.toLowerCase()}.${format}`;
    },
    async updateRfzo() {
      await delay(900);
      if (state.document?.kind !== "medical") throw "error.notMedicalCard";
      state.document.data.ValidUntil = "31.12.2027.";
      state.readId++;
      emit();
      return clone(state.document);
    },
    async pinTriesLeft() {
      if (state.document?.kind !== "id") throw "error.notIdCard";
      return 3;
    },
    async changePin(oldPin) {
      await delay(700);
      if (oldPin === "0000") throw "error.wrongPin:2";
    },
    async listCertificates() {
      await delay(300);
      if (state.document?.kind !== "id") throw "error.notIdCard";
      return clone(sampleCerts);
    },
    async saveCertificate() {
      await delay(200);
    },
    async getSettings() {
      return clone(settings);
    },
    async saveSettings(s) {
      settings = clone(s);
      try {
        localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
      } catch {
        /* ignore */
      }
    },
    async checkUpdate() {
      await delay(600);
      return {
        current: "0.1.0",
        latest: "0.2.0",
        newer: true,
        url: "https://github.com/vahidzekic/Chitach/releases/latest",
      };
    },
    async openPath(path) {
      console.info("[mock] open_path", path);
    },
    async openUrl(url) {
      window.open(url, "_blank", "noopener");
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
      return mockFinesInfo(state.document?.kind === "vehicle" ? state.document.data.RegistrationNumberOfVehicle : "");
    },
    async finesCheck(providerId, force = false) {
      if (state.document?.kind !== "vehicle") throw "error.notVehicleCard";
      return mockFinesCheck(state.document.data.RegistrationNumberOfVehicle, providerId, force, settings);
    },
    async historyInfo() {
      return mockHistoryInfo(state.document?.kind === "vehicle" ? state.document.data : null);
    },
    async historySetApiKey(providerId, apiKey, secret) {
      await mockHistorySetKey(providerId, apiKey, secret);
    },
    async historyRemoveApiKey(providerId) {
      await mockHistoryRemoveKey(providerId);
    },
    async historyApiCheck(providerId, operation, force = false) {
      if (state.document?.kind !== "vehicle") throw "error.notVehicleCard";
      return mockHistoryApiCheck(state.document.data, providerId, operation, force, settings);
    },

    async onAppState(cb) {
      stateListeners.add(cb);
      return () => stateListeners.delete(cb);
    },
    async onNotify(cb) {
      notifyListeners.add(cb);
      return () => notifyListeners.delete(cb);
    },
    async onSmartboxStatus(cb) {
      smartboxListeners.add(cb);
      return () => smartboxListeners.delete(cb);
    },

    async saveDialog(opts) {
      const name = baseName(opts.defaultPath ?? "dokument");
      const dir = settings.lastDir || "/home/korisnik/Dokumenti";
      return `${dir}/${name}`;
    },
    async openFileDialog() {
      return "/home/korisnik/Dokumenti/PPPDV-2026-09.xml";
    },
    async pickDirectory() {
      return "/home/korisnik/Dokumenti/Chitach";
    },
  };

  return api;
}
