// Minimal reactive i18n. Translation files are derived from Bas Celik's
// (assets/translations, MIT) with additional Chitach keys.
import type { Language } from "../api/types";
import srLatn from "./sr-Latn.json";
import srCyrl from "./sr-Cyrl.json";
import en from "./en.json";
import ru from "./ru.json";

type Dict = Record<string, string>;

const dictionaries: Record<Language, Dict> = {
  "sr-Latn": srLatn,
  "sr-Cyrl": srCyrl,
  en,
  ru,
};

export const languageNames: Record<Language, string> = {
  "sr-Latn": "Srpski (latinica)",
  "sr-Cyrl": "Српски (ћирилица)",
  en: "English",
  ru: "Русский",
};

const htmlLang: Record<Language, string> = {
  "sr-Latn": "sr-Latn",
  "sr-Cyrl": "sr-Cyrl",
  en: "en",
  ru: "ru",
};

const i18n = $state<{ lang: Language }>({ lang: "sr-Latn" });

export function setLanguage(lang: Language) {
  i18n.lang = dictionaries[lang] ? lang : "sr-Latn";
  document.documentElement.lang = htmlLang[i18n.lang];
}

export function currentLanguage(): Language {
  return i18n.lang;
}

export function hasKey(key: string): boolean {
  return key in dictionaries[i18n.lang] || key in dictionaries.en;
}

/** Translate `key`; `%s` placeholders are replaced by `args` in order. */
export function t(key: string, ...args: (string | number)[]): string {
  let s = dictionaries[i18n.lang][key] ?? dictionaries.en[key] ?? key;
  for (const a of args) s = s.replace("%s", String(a));
  return s;
}

/**
 * Backend errors are `error.key`, `error.key:N` or `error.key|detail`
 * (any translation key may be used). Returns translated title + optional detail.
 */
export function translateError(e: unknown): { title: string; detail?: string } {
  const raw = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
  let key = raw;
  let detail: string | undefined;
  const bar = raw.indexOf("|");
  if (bar >= 0) {
    key = raw.substring(0, bar);
    detail = raw.substring(bar + 1).trim() || undefined;
  }
  let arg: string | undefined;
  const m = /^([\w.-]+):(\d+)$/.exec(key);
  if (m) {
    key = m[1];
    arg = m[2];
  }
  if (/^[\w-]+(\.[\w-]+)+$/.test(key) && hasKey(key)) {
    if (key === "error.wrongPin" && arg === "0") return { title: t("error.pinBlocked"), detail };
    return { title: arg !== undefined ? t(key, arg) : t(key), detail };
  }
  // Not a known key: show raw text as detail under a generic title.
  return { title: t("error.error"), detail: raw };
}

export function yesNo(v: boolean): string {
  return v ? t("ui.yes") : t("ui.no");
}
