// Chitach Pro in the UI (see ../pro-api.ts). Official builds bundle this module as `$pro`.
import type { ProModule } from "../pro-api";
import CertificatesPage from "./CertificatesPage.svelte";
import ProLock from "./ProLock.svelte";
import ProSettings from "./ProSettings.svelte";
import SmartboxPage from "./SmartboxPage.svelte";
import VehiclePanels from "./VehiclePanels.svelte";
import XmlSignPage from "./XmlSignPage.svelte";
import { refreshLicense } from "./license";
import { messages } from "./i18n";

export const pro: ProModule = {
  available: true,
  pages: { certificates: CertificatesPage, xmlSign: XmlSignPage, smartbox: SmartboxPage },
  VehiclePanels,
  ProLock,
  SettingsSections: ProSettings,
  messages,
  init: refreshLicense,
};
