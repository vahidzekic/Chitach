# API ugovor: Tauri komande i događaji (Rust ↔ UI)

Ovaj dokument je **izvor istine** za komunikaciju između `src-tauri` i `ui/`.
Komande se pozivaju iz TS-a sa `invoke("ime_komande", { argumentiCamelCase })`.
Sve greške se vraćaju kao `string` (poruka greške, ključ prevoda ako počinje sa `error.`).

## Tipovi (TypeScript, definisani u `ui/src/lib/api/types.ts`)

```ts
export type Language = "sr-Latn" | "sr-Cyrl" | "en" | "ru";
export type Theme = "system" | "light" | "dark";
export type Mode = "reader" | "smartbox" | "xmlSign";
export type CardVendor = "mup" | "pks" | "posta" | "halcom" | "esmart";

export interface Settings {
  language: Language;
  theme: Theme;
  mode: Mode;
  pdfCyrillicLabels: boolean;
  autoSave: "off" | "save" | "saveAndOpen";
  autoSaveDir: string;
  runInBackground: boolean;   // system tray
  checkUpdates: boolean;
  lastDir: string;
  pkcs11Paths: Record<CardVendor, string>;   // "" = podrazumevano
}

export interface ReaderStatus { name: string; cardPresent: boolean; atr: string | null; }

// Document = tačan JSON izlaz celik_core::Document (serde tag/content):
//   { kind: "id",      data: IdDocument }       (PascalCase polja, Portrait = base64 JPEG)
//   { kind: "medical", data: MedicalDocument }
//   { kind: "vehicle", data: VehicleDocument }
export type Document =
  | { kind: "id"; data: IdDocument }
  | { kind: "medical"; data: MedicalDocument }
  | { kind: "vehicle"; data: VehicleDocument };

export type CardStatus =
  | "noReader"      // nema čitača
  | "waiting"       // čitač postoji, nema kartice
  | "reading"       // čitanje u toku
  | "ready"         // dokument pročitan
  | "unknownCard"   // nepoznata kartica (atr popunjen)
  | "error";        // greška (error popunjen)

export interface AppState {
  status: CardStatus;
  readers: ReaderStatus[];
  selectedReader: string | null;
  document: Document | null;
  atr: string | null;
  error: string | null;
  /** sekvenca koja raste pri svakom novom čitanju */
  readId: number;
}

export interface CertInfo { index: number; label: string; subject: string; issuer: string; notAfter: string; }
export interface Slot { id: number; description: string; }
export interface TokenCert { id: string /* hex */; label: string; subject: string; issuer: string; notAfter: string; }
export interface SmartboxStatus { running: boolean; address: string | null; vendors: CardVendor[]; error: string | null; }
export interface AppInfo { version: string; os: "linux" | "macos" | "windows"; arch: string; }
export interface UpdateInfo { current: string; latest: string; newer: boolean; url: string; }
```

## Komande

| Komanda | Argumenti | Vraća | Opis |
|---|---|---|---|
| `get_app_info` | – | `AppInfo` | |
| `get_state` | – | `AppState` | trenutno stanje (UI ga poziva pri startu) |
| `select_reader` | `{ name: string \| null }` | `void` | `null` = automatski (prvi sa karticom) |
| `reload_card` | – | `void` | ponovo pročitaj karticu |
| `export_document` | `{ format: "pdf"\|"json"\|"xlsx", path: string }` | `string` (putanja) | snima trenutni dokument |
| `suggest_file_name` | `{ format }` | `string` | npr. `petar_petrovic.pdf` |
| `update_rfzo` | – | `Document` | ažurira "overena do" za zdravstvenu |
| `pin_tries_left` | – | `number` | samo lična karta |
| `change_pin` | `{ oldPin, newPin }` | `void` | greška `error.wrongPin:N` (N preostalih pokušaja) |
| `list_certificates` | – | `CertInfo[]` | sertifikati sa lične karte |
| `save_certificate` | `{ index, path }` | `void` | DER/.cer |
| `get_settings` | – | `Settings` | |
| `save_settings` | `{ settings: Settings }` | `void` | primenjuje temu/jezik odmah, mod nakon restarta |
| `check_update` | – | `UpdateInfo` | GitHub releases `vahidzekic/Chitach` |
| `open_path` | `{ path }` | `void` | otvara fajl u podrazumevanom programu |
| `sign_load_modules` | – | `CardVendor[]` | učitava PKCS#11 module iz podešavanja |
| `sign_list_slots` | `{ vendor }` | `Slot[]` | |
| `sign_login` | `{ vendor, slot, pin }` | `TokenCert[]` | |
| `sign_xml_file` | `{ certId, inputPath, outputPath }` | `void` | |
| `sign_logout` | – | `void` | |
| `smartbox_status` | – | `SmartboxStatus` | |

## Događaji (backend → UI, `listen(...)`)

| Događaj | Payload |
|---|---|
| `app-state` | `AppState` (emituje se pri svakoj promeni) |
| `notify` | `{ level: "info" \| "success" \| "error", key: string, detail?: string }` (npr. auto-čuvanje) |
| `smartbox-status` | `SmartboxStatus` |

## Mock režim

Ako se UI pokrene van Tauri-ja (`npm run dev` u browseru), `ui/src/lib/api` koristi mock
implementaciju sa primer dokumentima — za razvoj i screenshot-ove.
Backend: `CHITACH_MOCK=1` emituje primer dokumenta bez čitača.
