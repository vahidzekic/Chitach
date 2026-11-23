// Mock of the vehicle history ("Istorija vozila") backend: same definitions
// (src-tauri/providers/history.json, wmi.json) and rules as src-tauri/src/history.rs,
// fake API answers. API keys are kept in memory only.
import definitions from "../../../../src-tauri/providers/history.json";
import wmiTable from "../../../../src-tauri/providers/wmi.json";
import type {
  HistoryApiResult,
  HistoryCheck,
  HistoryCheckLevel,
  HistoryInfo,
  HistoryProvider,
  Settings,
  VehicleDocument,
  VinDecoded,
} from "./types";
import { normalizePlate } from "./mockFines";

interface Definition {
  id: string;
  category: string;
  name: string;
  kind: string;
  key?: string;
  price?: string;
  url: string;
  altUrl?: string;
  partner?: boolean;
  api?: { service: string; base: string; operations: string[] };
}

interface Manufacturer {
  wmi: string;
  make: string;
  names?: string[];
  country?: string;
  modelYear?: boolean;
}

const ORDER = "ABCDEFGHJKLMNPRSTUVWXYZ1234567890";
const YEAR_CODES = "ABCDEFGHJKLMNPRSTVWXY123456789";
const TRANSLIT: Record<string, number> = {
  A: 1, B: 2, C: 3, D: 4, E: 5, F: 6, G: 7, H: 8, J: 1, K: 2, L: 3, M: 4, N: 5, P: 7, R: 9, S: 2, T: 3, U: 4,
  V: 5, W: 6, X: 7, Y: 8, Z: 9,
};
const WEIGHTS = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];

export const normalizeVin = (vin: string) => vin.replace(/[^a-z0-9]/gi, "").toUpperCase();

function vinFormat(vin: string): HistoryInfo["vinFormat"] {
  if (!vin) return "empty";
  return /^[A-HJ-NPR-Z0-9]{17}$/.test(vin) ? "valid" : "invalid";
}

function checkDigit(vin: string): string {
  const sum = [...vin].reduce((s, c, i) => s + (/\d/.test(c) ? Number(c) : (TRANSLIT[c] ?? 0)) * WEIGHTS[i], 0);
  const r = sum % 11;
  return r === 10 ? "X" : String(r);
}

function modelYear(code: string, currentYear: number): number | null {
  const i = YEAR_CODES.indexOf(code);
  if (i < 0) return null;
  let y = 1980 + i;
  while (y + 30 <= currentYear + 1) y += 30;
  return y;
}

function region(vin: string): string | null {
  const b = ORDER.indexOf(vin[1]);
  const r = wmiTable.regions.find(
    (r) => r.from[0] === vin[0] && b >= ORDER.indexOf(r.from[1]) && b <= ORDER.indexOf(r.to[1]),
  );
  return r?.country ?? null;
}

const manufacturer = (vin: string) =>
  (wmiTable.manufacturers as Manufacturer[]).find((m) => m.wmi === vin.substring(0, 3));

export function normalizeMake(make: string): string {
  return normalizePlate(make)
    .replace(/[ČĆ]/g, "C")
    .replace(/Š/g, "S")
    .replace(/Ž/g, "Z")
    .replace(/Đ/g, "D")
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

function yearOf(s: string): number | null {
  const ys = (s.match(/\d+/g) ?? []).filter((p) => p.length === 4).map(Number).filter((y) => y >= 1900 && y <= 2100);
  return ys.length ? ys[ys.length - 1] : null;
}

function decode(vin: string, currentYear: number): VinDecoded | null {
  if (vinFormat(vin) !== "valid") return null;
  const m = manufacturer(vin);
  const na = /^[1-5]/.test(vin);
  return {
    wmi: vin.substring(0, 3),
    vds: vin.substring(3, 9),
    vis: vin.substring(9),
    make: m?.make ?? null,
    country: m?.country ?? region(vin),
    northAmerica: na,
    modelYear: na || m?.modelYear ? modelYear(vin[9], currentYear) : null,
    checkDigitOk: na ? checkDigit(vin) === vin[8] : null,
  };
}

const c = (id: string, level: HistoryCheckLevel, variant: string, args: (string | number)[] = []): HistoryCheck => ({
  id,
  level,
  variant,
  args: args.map(String),
});

function checks(v: VehicleDocument, vin: string, d: VinDecoded | null, currentYear: number): HistoryCheck[] {
  const out: HistoryCheck[] = [];
  const fmt = vinFormat(vin);
  if (fmt === "empty") return [c("vin", "unknown", "empty")];
  out.push(fmt === "valid" ? c("vin", "ok", "valid") : c("vin", "warning", "invalid", [vin.length]));
  const produced = yearOf(v.YearOfProduction);
  if (d) {
    const m = manufacturer(vin);
    const cardMake = normalizeMake(v.VehicleMake);
    if (!m) out.push(c("make", "unknown", "unknownWmi", [d.wmi]));
    else if (cardMake) {
      const ok = (m.names ?? []).some((n) => n && (cardMake.includes(n) || n.includes(cardMake)));
      out.push(ok ? c("make", "ok", "match", [m.make]) : c("make", "warning", "mismatch", [m.make, v.VehicleMake]));
    }
    if (d.modelYear === null) out.push(c("modelYear", "unknown", "notEncoded"));
    else if (produced === null) out.push(c("modelYear", "info", "only", [d.modelYear]));
    else if (Math.abs(d.modelYear - produced) <= 1) out.push(c("modelYear", "ok", "match", [d.modelYear]));
    else out.push(c("modelYear", "warning", "mismatch", [d.modelYear, produced]));
    if (d.checkDigitOk !== null)
      out.push(d.checkDigitOk ? c("checkDigit", "ok", "valid") : c("checkDigit", "warning", "invalid"));
  }
  const first = yearOf(v.DateOfFirstRegistration);
  if (produced !== null && first !== null) {
    if (first < produced) out.push(c("firstRegistration", "warning", "beforeProduction", [first, produced]));
    else if (first - produced >= 2) out.push(c("firstRegistration", "info", "late", [first - produced]));
    else out.push(c("firstRegistration", "ok", "new"));
  }
  if (produced !== null && currentYear - produced >= 0) out.push(c("age", "info", "years", [currentYear - produced]));
  return out;
}

const keys = new Map<string, { apiKey: string; secret: string }>();

export function mockHistoryInfo(v: VehicleDocument | null): HistoryInfo {
  const year = new Date().getFullYear();
  const vin = normalizeVin(v?.VehicleIdNumber ?? "");
  const plate = normalizePlate(v?.RegistrationNumberOfVehicle ?? "");
  const decoded = decode(vin, year);
  const resolve = (u: string) => u.replace("{vin}", encodeURIComponent(vin)).replace("{plate}", encodeURIComponent(plate));
  const providers: HistoryProvider[] = (definitions.providers as Definition[])
    .filter((d) => d.kind === "link" || d.kind === "api")
    .map((d) => ({
      id: d.id,
      name: d.name,
      category: d.category as HistoryProvider["category"],
      kind: d.kind as HistoryProvider["kind"],
      key: (d.key ?? "vin") as HistoryProvider["key"],
      price: d.price ?? null,
      url: resolve(d.url),
      altUrl: d.altUrl ? resolve(d.altUrl) : null,
      keyInUrl: d.url.includes(d.key === "plate" ? "{plate}" : "{vin}"),
      partner: !!d.partner,
      operations: d.api?.operations ?? [],
      apiDomain: d.api ? (/^https:\/\/([^/?#]+)/.exec(d.api.base)?.[1] ?? null) : null,
      configured: d.kind === "api" && keys.has(d.id),
    }));
  return {
    vin: v?.VehicleIdNumber ?? "",
    normalizedVin: vin,
    vinFormat: vinFormat(vin),
    plate: v?.RegistrationNumberOfVehicle ?? "",
    normalizedPlate: plate,
    make: v?.VehicleMake ?? "",
    model: v?.CommercialDescription ?? "",
    yearOfProduction: v?.YearOfProduction ?? "",
    dateOfFirstRegistration: v?.DateOfFirstRegistration ?? "",
    decoded,
    checks: v ? checks(v, vin, decoded, year) : [],
    providers,
  };
}

export async function mockHistorySetKey(providerId: string, apiKey: string, secret: string) {
  if (!(definitions.providers as Definition[]).some((d) => d.id === providerId && d.kind === "api"))
    throw "error.historyNotApi";
  if (!apiKey.trim()) throw "error.historyNoKey";
  keys.set(providerId, { apiKey: apiKey.trim(), secret: secret.trim() });
  for (const k of [...cache.keys()]) if (k.startsWith(`${providerId}|`)) cache.delete(k);
}

export async function mockHistoryRemoveKey(providerId: string) {
  keys.delete(providerId);
  for (const k of [...cache.keys()]) if (k.startsWith(`${providerId}|`)) cache.delete(k);
}

const SAMPLE: Record<string, { price: string; fields: [string, string][] }> = {
  decode: {
    price: "0.25 EUR",
    fields: [
      ["Make", "Škoda"],
      ["Model", "Octavia"],
      ["Model Year", "2020"],
      ["Body", "Combi"],
      ["Engine Displacement (ccm)", "1968"],
      ["Engine Power (kW)", "110"],
      ["Fuel Type - Primary", "Diesel"],
      ["Transmission", "Manual"],
      ["Drive", "Front-wheel drive"],
      ["Plant Country", "Czech Republic"],
    ],
  },
  "stolen-check": {
    price: "0.49 EUR",
    fields: [
      ["stolen.status", "not reported as stolen"],
      ["stolen.sources", "EU police databases (sample)"],
    ],
  },
  "vehicle-market-value": {
    price: "0.99 EUR",
    fields: [
      ["vehicle.make", "Škoda"],
      ["vehicle.model", "Octavia"],
      ["prices.below", "12900"],
      ["prices.mean", "14650"],
      ["prices.above", "16400"],
      ["prices.currency", "EUR"],
      ["records[0].odometer", "118000"],
      ["records[1].odometer", "96000"],
    ],
  },
};

const cache = new Map<string, HistoryApiResult>();

export async function mockHistoryApiCheck(
  v: VehicleDocument,
  providerId: string,
  operation: string,
  force: boolean,
  settings: Settings,
): Promise<HistoryApiResult> {
  const d = (definitions.providers as Definition[]).find((x) => x.id === providerId);
  if (!d) throw `error.historyUnknownProvider|${providerId}`;
  if (d.kind !== "api" || !d.api) throw "error.historyNotApi";
  if (!d.api.operations.includes(operation)) throw `error.historyUnknownOperation|${operation}`;
  if (!settings.history.consentGiven) throw "error.historyConsent";
  if (!keys.has(providerId)) throw "error.historyNoKey";
  const vin = normalizeVin(v.VehicleIdNumber);
  const key = `${providerId}|${operation}|${vin}`;
  const hit = cache.get(key);
  const age = hit ? Date.now() - hit.checkedAt : Infinity;
  if (hit && age < (force || hit.status !== "ok" ? 60_000 : 30 * 60_000)) return { ...hit, cached: true };
  await new Promise((r) => setTimeout(r, 900));
  const forced = new URLSearchParams(location.search).get("history");
  const sample = SAMPLE[operation] ?? SAMPLE.decode;
  const status = (forced ?? "ok") as HistoryApiResult["status"];
  const r: HistoryApiResult = {
    providerId,
    operation,
    status,
    fields: status === "ok" ? structuredClone(sample.fields) : [],
    price: status === "ok" ? sample.price : null,
    detail: status === "unauthorized" ? "HTTP 403" : status === "error" ? "Insufficient balance" : null,
    checkedAt: Date.now(),
    cached: false,
  };
  cache.set(key, r);
  return r;
}
