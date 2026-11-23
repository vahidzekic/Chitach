// Translations of the Chitach Pro UI, merged into the app's dictionaries.
import type { Language } from "./types";
import srLatn from "./i18n/sr-Latn.json";
import srCyrl from "./i18n/sr-Cyrl.json";
import en from "./i18n/en.json";
import ru from "./i18n/ru.json";

export const messages: Record<Language, Record<string, string>> = {
  "sr-Latn": srLatn,
  "sr-Cyrl": srCyrl,
  en,
  ru,
};
