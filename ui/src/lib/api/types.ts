// Types shared between the Rust backend (src-tauri) and the UI.
// Source of truth: docs/04-api-ugovor.md. Document field names follow the
// serde (PascalCase) names of crates/celik-core/src/document.rs.

export type Language = "sr-Latn" | "sr-Cyrl" | "en" | "ru";
export type Theme = "system" | "light" | "dark";
export type Mode = "reader" | "smartbox" | "xmlSign";
export type CardVendor = "mup" | "pks" | "posta" | "halcom" | "esmart";
export type AutoSave = "off" | "save" | "saveAndOpen";
export type ExportFormat = "pdf" | "json" | "xlsx";

export const CARD_VENDORS: CardVendor[] = ["mup", "pks", "posta", "halcom", "esmart"];
export const LANGUAGES: Language[] = ["sr-Latn", "sr-Cyrl", "en", "ru"];

export interface Settings {
  language: Language;
  theme: Theme;
  mode: Mode;
  pdfCyrillicLabels: boolean;
  autoSave: AutoSave;
  autoSaveDir: string;
  /** system tray */
  runInBackground: boolean;
  checkUpdates: boolean;
  lastDir: string;
  /** "" = default module path */
  pkcs11Paths: Record<CardVendor, string>;
  /** fines check ("Provera kazni") */
  fines: FinesSettings;
  /** vehicle history ("Istorija vozila"); API keys are not part of the settings */
  history: HistorySettings;
}

export interface HistorySettings {
  /** the user agreed that the VIN is sent to the configured history APIs */
  consentGiven: boolean;
}

export interface FinesSettings {
  /** run the automatic check when a vehicle card is read */
  autoCheck: boolean;
  /** the user agreed that the plate is sent to the automatic-check portals */
  consentGiven: boolean;
  /** automatic-check providers that may be used; missing = all */
  enabledProviders?: string[];
  /** download newer signed provider definitions from GitHub (at most daily) */
  remoteDefinitions: boolean;
}

export interface ReaderStatus {
  name: string;
  cardPresent: boolean;
  atr: string | null;
}

/** ID card (Gemalto / Apollo), identity card for foreigners, residence permit. */
export interface IdDocument {
  /** base64 encoded JPEG */
  Portrait: string;
  DocRegNo: string;
  /** "" (Apollo) | "ID" | "IF" | "RP" */
  DocumentType: string;
  IssuingDate: string;
  ExpiryDate: string;
  IssuingAuthority: string;
  DocumentSerialNumber: string;
  ChipSerialNumber: string;
  DocumentName: string;
  PersonalNumber: string;
  Surname: string;
  GivenName: string;
  ParentGivenName: string;
  Sex: string;
  PlaceOfBirth: string;
  CommunityOfBirth: string;
  StateOfBirth: string;
  StateOfBirthCode: string;
  DateOfBirth: string;
  StatusOfForeigner: string;
  NationalityFull: string;
  PurposeOfStay: string;
  ENote: string;
  State: string;
  Community: string;
  Place: string;
  Street: string;
  HouseNumber: string;
  HouseLetter: string;
  Entrance: string;
  Floor: string;
  ApartmentNumber: string;
  AddressDate: string;
  AddressLabel: string;
}

/** Medical insurance card (RFZO). */
export interface MedicalDocument {
  InsurerName: string;
  InsurerID: string;
  CardId: string;
  DateOfIssue: string;
  DateOfExpiry: string;
  ChipSerialNumber: string;
  PrintLanguage: string;
  PersonalNumber: string;
  FamilyNameLatin: string;
  GivenNameLatin: string;
  ParentNameLatin: string;
  FamilyName: string;
  GivenName: string;
  ParentName: string;
  Gender: string;
  InsurantNumber: string;
  DateOfBirth: string;
  Apartment: string;
  Number: string;
  Entrance: string;
  Street: string;
  StreetCode: string;
  Place: string;
  PostNumber: string;
  Municipality: string;
  Country: string;
  ValidUntil: string;
  PermanentlyValid: boolean;
  CarrierGivenNameLatin: string;
  CarrierFamilyNameLatin: string;
  CarrierGivenName: string;
  CarrierFamilyName: string;
  CarrierIdNumber: string;
  CarrierInsurantNumber: string;
  CarrierFamilyMember: boolean;
  CarrierRelationship: string;
  InsuranceBasisRZZO: string;
  InsuranceStartDate: string;
  InsuranceDescription: string;
  TaxpayerName: string;
  TaxpayerResidence: string;
  TaxpayerNumber: string;
  TaxpayerIdNumber: string;
  TaxpayerActivityCode: string;
}

/** Vehicle registration certificate. */
export interface VehicleDocument {
  AuthorityIssuing: string;
  ColourOfVehicle: string;
  CommercialDescription: string;
  CompetentAuthority: string;
  DateOfFirstRegistration: string;
  EngineCapacity: string;
  EngineIdNumber: string;
  ExpiryDate: string;
  HomologationMark: string;
  IssuingDate: string;
  MaximumNetPower: string;
  MaximumPermissibleLadenMass: string;
  NumberOfAxles: string;
  NumberOfSeats: string;
  NumberOfStandingPlaces: string;
  OwnerAddress: string;
  OwnerName: string;
  OwnersPersonalNo: string;
  OwnersSurnameOrBusinessName: string;
  PowerWeightRatio: string;
  RegistrationNumberOfVehicle: string;
  SerialNumber: string;
  StateIssuing: string;
  TypeApprovalNumber: string;
  TypeOfFuel: string;
  UnambiguousNumber: string;
  UsersAddress: string;
  UsersName: string;
  UsersPersonalNo: string;
  UsersSurnameOrBusinessName: string;
  VehicleCategory: string;
  VehicleIdNumber: string;
  VehicleLoad: string;
  VehicleMake: string;
  VehicleMass: string;
  VehicleType: string;
  YearOfProduction: string;
}

/** Exact JSON output of celik_core::Document (serde tag/content). */
export type Document =
  | { kind: "id"; data: IdDocument }
  | { kind: "medical"; data: MedicalDocument }
  | { kind: "vehicle"; data: VehicleDocument };

export type DocumentKind = Document["kind"];

export type CardStatus =
  | "noReader" // no reader connected
  | "waiting" // reader present, no card
  | "reading" // reading in progress
  | "ready" // document read
  | "unknownCard" // unknown card (atr filled)
  | "error"; // error (error filled)

export interface AppState {
  status: CardStatus;
  readers: ReaderStatus[];
  selectedReader: string | null;
  document: Document | null;
  atr: string | null;
  error: string | null;
  /** sequence that increases with every new read */
  readId: number;
}

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

export interface AppInfo {
  version: string;
  os: "linux" | "macos" | "windows";
  arch: string;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  newer: boolean;
  url: string;
}

export type FinesCategory = "parking" | "person" | "toll";
/** "link": opened in the browser; "json_get" / "html_form": optional automatic check */
export type FinesProviderKind = "link" | "json_get" | "html_form";

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

export interface NotifyEvent {
  level: "info" | "success" | "error";
  key: string;
  detail?: string;
}

export interface FileFilter {
  name: string;
  extensions: string[];
}

export type Unlisten = () => void;

/** Everything the UI needs from the host. Implemented by Tauri and by the mock. */
export interface Api {
  readonly isMock: boolean;

  getAppInfo(): Promise<AppInfo>;
  getState(): Promise<AppState>;
  selectReader(name: string | null): Promise<void>;
  reloadCard(): Promise<void>;
  exportDocument(format: ExportFormat, path: string): Promise<string>;
  suggestFileName(format: ExportFormat): Promise<string>;
  updateRfzo(): Promise<Document>;
  pinTriesLeft(): Promise<number>;
  changePin(oldPin: string, newPin: string): Promise<void>;
  listCertificates(): Promise<CertInfo[]>;
  saveCertificate(index: number, path: string): Promise<void>;
  getSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<void>;
  checkUpdate(): Promise<UpdateInfo>;
  openPath(path: string): Promise<void>;
  openUrl(url: string): Promise<void>;
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

  onAppState(cb: (s: AppState) => void): Promise<Unlisten>;
  onNotify(cb: (n: NotifyEvent) => void): Promise<Unlisten>;
  onSmartboxStatus(cb: (s: SmartboxStatus) => void): Promise<Unlisten>;

  /** Native dialogs. Return null when cancelled. */
  saveDialog(opts: { defaultPath?: string; filters?: FileFilter[]; title?: string }): Promise<string | null>;
  openFileDialog(opts: { defaultPath?: string; filters?: FileFilter[]; title?: string }): Promise<string | null>;
  pickDirectory(opts: { defaultPath?: string; title?: string }): Promise<string | null>;
}
