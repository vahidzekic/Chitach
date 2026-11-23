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

export interface AppInfo {
  version: string;
  os: "linux" | "macos" | "windows" | "android" | "ios";
  arch: string;
  /** Chitach Pro is compiled into the app (official builds); false = Chitach Free.
   * Missing in the browser mock, where the bundled UI decides. */
  pro?: boolean;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  newer: boolean;
  url: string;
}

export interface LicenseInfo {
  /** Pro gating is on in this build; when false the features are free and Pro UI is hidden */
  enabled: boolean;
  active: boolean;
  /** a key is stored (it may be inactive when it could not be validated for too long) */
  stored: boolean;
  /** last 4 characters of the stored key, e.g. "…A1B2" */
  keyHint: string | null;
  email: string | null;
  /** unix time in ms of the last successful validation */
  validatedAt: number | null;
  /** checkout page, when configured */
  buyUrl: string | null;
  /** everything Chitach Pro unlocks (reading the three cards and PDF export stay free) */
  features: ProFeature[];
}

export type ProFeature =
  | "dataExport"
  | "autoSave"
  | "background"
  | "pin"
  | "certificates"
  | "xmlSign"
  | "smartbox"
  | "fines"
  | "history";

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
  getSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<void>;
  checkUpdate(): Promise<UpdateInfo>;
  openPath(path: string): Promise<void>;
  openUrl(url: string): Promise<void>;

  onAppState(cb: (s: AppState) => void): Promise<Unlisten>;
  onNotify(cb: (n: NotifyEvent) => void): Promise<Unlisten>;

  /** Native dialogs. Return null when cancelled. */
  saveDialog(opts: { defaultPath?: string; filters?: FileFilter[]; title?: string }): Promise<string | null>;
  openFileDialog(opts: { defaultPath?: string; filters?: FileFilter[]; title?: string }): Promise<string | null>;
  pickDirectory(opts: { defaultPath?: string; title?: string }): Promise<string | null>;
}
