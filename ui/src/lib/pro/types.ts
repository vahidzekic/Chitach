// Types of the Chitach Pro commands (see docs/04-api-ugovor.md).
export * from "../api/types";
import type { CardVendor } from "../api/types";

export interface CertInfo {
  index: number;
  label: string;
  subject: string;
  issuer: string;
  notAfter: string;
}

export interface Slot {
  id: number;
  description: string;
}

export interface TokenCert {
  /** hex */
  id: string;
  label: string;
  subject: string;
  issuer: string;
  notAfter: string;
}

export interface SmartboxStatus {
  running: boolean;
  address: string | null;
  vendors: CardVendor[];
  error: string | null;
}

export type FinesCategory = "parking" | "person" | "toll";
/** "link": opened in the browser; "json_get" / "html_form" / "json_post": optional automatic check */
export type FinesProviderKind = "link" | "json_get" | "html_form" | "json_post";

export interface FinesProvider {
  id: string;
  /** fallback name; UI uses `fines.provider.<id>.name` when translated */
  name: string;
  category: FinesCategory;
  /** plate city codes served, e.g. ["BG"] */
  cities: string[];
  kind: FinesProviderKind;
  /** has an automatic check */
  auto: boolean;
  /** page to open (plate filled in where supported) */
  url: string;
  plateInUrl: boolean;
  /** host the plate is sent to by the automatic check */
  checkDomain: string | null;
  /** serves the city of the plate */
  matchesCity: boolean;
}

export interface FinesInfo {
  plate: string;
  normalizedPlate: string;
  cityCode: string | null;
  providers: FinesProvider[];
  /** provider definitions in use: built in, or a newer signed download */
  definitions?: { source: "embedded" | "remote"; updated: string };
}

export type FinesStatus = "clean" | "found" | "manual" | "unavailable" | "parseError";

export interface FineItem {
  date: string | null;
  amount: string | null;
  location: string | null;
  description: string | null;
  /** all scalar fields of the response item */
  fields: Record<string, string>;
}

export interface FinesCheckResult {
  providerId: string;
  status: FinesStatus;
  items: FineItem[];
  detail: string | null;
  /** unix time in ms */
  checkedAt: number;
  cached: boolean;
}

export type HistoryCategory = "domestic" | "foreign" | "usa" | "api";
/** "link": opened in the browser; "api": report through an API with the user's own key */
export type HistoryProviderKind = "link" | "api";

export interface HistoryProvider {
  id: string;
  /** fallback name; UI uses `history.provider.<id>.name` when translated */
  name: string;
  category: HistoryCategory;
  kind: HistoryProviderKind;
  /** what the provider is searched by (copied when not in the URL) */
  key: "vin" | "plate";
  /** indicative price of a report, as published */
  price: string | null;
  url: string;
  /** second page, e.g. the iOS app */
  altUrl: string | null;
  keyInUrl: boolean;
  /** affiliate link (must be disclosed) */
  partner: boolean;
  /** api only */
  operations: string[];
  apiDomain: string | null;
  /** api only: a key is stored */
  configured: boolean;
}

export type HistoryCheckLevel = "ok" | "warning" | "info" | "unknown";

/** Translated with `history.check.<id>.<variant>` and `args`. */
export interface HistoryCheck {
  id: string;
  level: HistoryCheckLevel;
  variant: string;
  args: string[];
}

export interface VinDecoded {
  wmi: string;
  vds: string;
  vis: string;
  make: string | null;
  /** ISO 3166-1 alpha-2 */
  country: string | null;
  northAmerica: boolean;
  modelYear: number | null;
  /** null outside North America */
  checkDigitOk: boolean | null;
}

export interface HistoryInfo {
  vin: string;
  normalizedVin: string;
  vinFormat: "valid" | "invalid" | "empty";
  plate: string;
  normalizedPlate: string;
  make: string;
  model: string;
  yearOfProduction: string;
  dateOfFirstRegistration: string;
  decoded: VinDecoded | null;
  checks: HistoryCheck[];
  providers: HistoryProvider[];
}

export type HistoryApiStatus = "ok" | "unauthorized" | "error" | "unavailable" | "parseError";

export interface HistoryApiResult {
  providerId: string;
  operation: string;
  status: HistoryApiStatus;
  /** the provider's own labels and values, in order */
  fields: [string, string][];
  /** price of this call as reported by the API */
  price: string | null;
  detail: string | null;
  /** unix time in ms */
  checkedAt: number;
  cached: boolean;
}

/** Chitach Pro licence (unlocks the fines check and the vehicle history). */
