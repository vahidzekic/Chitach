// What Chitach Pro adds to the UI. The app imports the Pro module as `$pro`: in official
// builds that is `lib/pro` (Chitach Pro), otherwise `lib/pro-stub` (Chitach Free, which has
// none of these). See docs/11-free-i-pro.md.
import type { Component } from "svelte";
import type { Language, Settings } from "./api/types";

export interface ProModule {
  /** Pro is bundled with this UI. */
  readonly available: boolean;
  /** Pages: certificates and PIN change (ID card), XML signing and Smartbox modes. */
  readonly pages?: {
    certificates: Component;
    xmlSign: Component;
    smartbox: Component;
  };
  /** Shown under the vehicle card: fines check and vehicle history (or the Pro lock). */
  readonly VehiclePanels?: Component;
  /** Shown in place of a Pro page while Pro is not active. */
  readonly ProLock?: Component;
  /** Settings sections (Pro licence, fines, history, PKCS#11 modules); edits `draft`. */
  readonly SettingsSections?: Component<{ draft: Settings; isMobile: boolean }>;
  /** Extra translations, merged into the app's dictionaries. */
  readonly messages?: Partial<Record<Language, Record<string, string>>>;
  /** Load what Pro needs at start (the licence). */
  init?(): Promise<void>;
}
