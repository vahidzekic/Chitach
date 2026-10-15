// Mock of the fines check ("Provera kazni") backend: same provider definitions
// (src-tauri/providers/fines.json) and plate rules as src-tauri/src/fines.rs, fake
// automatic-check results (deterministic by plate: even digit sum = two tickets).
import definitions from "../../../../src-tauri/providers/fines.json";
import type { FineItem, FinesCheckResult, FinesInfo, FinesProvider, FinesProviderKind, Settings } from "./types";

interface Definition {
  id: string;
  category: string;
  name: string;
  portal?: string;
  route?: string;
  cities?: string[];
  kind: string;
  url: string;
  check?: { url: string; parser: string };
}

const CYR: Record<string, string> = {
  А: "A", Б: "B", В: "V", Г: "G", Д: "D", Ђ: "Đ", Е: "E", Ж: "Ž", З: "Z", И: "I", Ј: "J", К: "K", Л: "L", Љ: "LJ",
  М: "M", Н: "N", Њ: "NJ", О: "O", П: "P", Р: "R", С: "S", Т: "T", Ћ: "Ć", У: "U", Ф: "F", Х: "H", Ц: "C", Ч: "Č",
  Џ: "DŽ", Ш: "Š",
};

export function normalizePlate(plate: string): string {
  return [...plate.toUpperCase()]
    .filter((c) => /[\p{L}\p{N}]/u.test(c))
    .map((c) => CYR[c] ?? c)
    .join("");
}

export function cityCode(normalized: string): string | null {
  const m = /^\p{L}+/u.exec(normalized);
  return m && [...m[0]].length === 2 ? m[0] : null;
}

function host(url: string): string | null {
  return /^https?:\/\/([^/?#]+)/.exec(url)?.[1] ?? null;
}

/** `{route}` / `{city}` placeholders of multi-city portals (see fines.rs `Provider::expand`). */
function expand(d: Definition, template: string): string {
  return template
    .replace("{route}", encodeURIComponent(d.route ?? ""))
    .replace("{city}", encodeURIComponent(d.cities?.[0] ?? ""));
}

export function mockFinesInfo(plate: string): FinesInfo {
  const normalized = normalizePlate(plate);
  const city = cityCode(normalized);
  const providers: FinesProvider[] = (definitions.providers as Definition[])
    .filter((d) => d.kind === "link" || d.kind === "json_get" || d.kind === "html_form")
    .map((d) => ({
      id: d.id,
      name: d.name,
      category: d.category as FinesProvider["category"],
      cities: d.cities ?? [],
      kind: d.kind as FinesProviderKind,
      auto: d.kind !== "link" && !!d.check,
      url: expand(d, d.url).replace("{plate}", encodeURIComponent(normalized)),
      plateInUrl: d.url.includes("{plate}"),
      checkDomain: d.kind !== "link" && d.check ? host(expand(d, d.check.url)) : null,
      matchesCity: !!city && (d.cities ?? []).includes(city),
    }));
  providers.sort((a, b) => Number(b.matchesCity) - Number(a.matchesCity));
  return {
    plate,
    normalizedPlate: normalized,
    cityCode: city,
    providers,
    definitions: { source: "embedded", updated: definitions.updated },
  };
}

const SAMPLE_ITEMS: FineItem[] = [
  {
    date: "03.10.2026. 10:42",
    amount: "2400.00",
    location: "Bulevar oslobođenja 12, zona 1",
    description: "ePPK – posebna parking karta",
    fields: { brojNaloga: "PPK-2026-118204" },
  },
  {
    date: "21.09.2026. 16:05",
    amount: "1200.00",
    location: "Zmaj Jovina 4, zona 0",
    description: "ePPK – prekoračenje vremena",
    fields: { brojNaloga: "PPK-2026-109377" },
  },
];

const cache = new Map<string, FinesCheckResult>();

export async function mockFinesCheck(
  plate: string,
  providerId: string,
  force: boolean,
  settings: Settings,
): Promise<FinesCheckResult> {
  const info = mockFinesInfo(plate);
  const p = info.providers.find((x) => x.id === providerId);
  if (!p) throw `error.finesUnknownProvider|${providerId}`;
  if (!p.auto) throw "error.finesNotAutomatic";
  if (!settings.fines.consentGiven) throw "error.finesConsent";
  if (settings.fines.enabledProviders && !settings.fines.enabledProviders.includes(providerId))
    throw "error.finesProviderDisabled";
  const key = `${providerId}|${info.normalizedPlate}`;
  const hit = cache.get(key);
  const age = hit ? Date.now() - hit.checkedAt : Infinity;
  if (hit && age < (force ? 60_000 : 30 * 60_000)) return { ...hit, cached: true };
  await new Promise((r) => setTimeout(r, 900));
  const forced = new URLSearchParams(location.search).get("fines");
  const sum = [...info.normalizedPlate].reduce((s, c) => s + (/\d/.test(c) ? Number(c) : 0), 0);
  const found = forced ? forced === "found" : sum % 2 === 0;
  const r: FinesCheckResult = {
    providerId,
    status: forced && forced !== "found" && forced !== "clean" ? (forced as FinesCheckResult["status"]) : found ? "found" : "clean",
    items: found ? structuredClone(SAMPLE_ITEMS) : [],
    detail: forced === "manual" ? "HTTP 403" : forced === "unavailable" ? "timeout" : null,
    checkedAt: Date.now(),
    cached: false,
  };
  if (r.status !== "found") r.items = [];
  cache.set(key, r);
  return r;
}
