import vi from "./locales/vi";

export type LocaleMode = "auto" | "en" | "vi";
export type ResolvedLocale = "en" | "vi";

const STORAGE_KEY = "purgekit.locale";

function detectSystemLocale(): ResolvedLocale {
  if (typeof navigator === "undefined") return "en";
  return navigator.language.toLowerCase().startsWith("vi") ? "vi" : "en";
}

class I18nController {
  mode = $state<LocaleMode>("en");
  resolved: ResolvedLocale = $derived(
    this.mode === "auto" ? detectSystemLocale() : this.mode
  );

  init() {
    // English-only for now; the dictionaries and setMode() stay ready for a
    // future language release.
    this.mode = "en";
    this.apply();
  }

  setMode(mode: LocaleMode) {
    this.mode = mode;
    try {
      localStorage.setItem(STORAGE_KEY, mode);
    } catch {}
    this.apply();
  }

  translate(key: string, params?: Record<string, string | number>): string {
    const template =
      this.resolved === "vi" ? vi[key] ?? key : key;
    if (!params) return template;
    return template.replace(/\{(\w+)\}/g, (match, name: string) => {
      const value = params[name];
      return value === undefined ? match : String(value);
    });
  }

  private apply() {
    if (typeof document !== "undefined") {
      document.documentElement.lang = this.resolved;
    }
  }
}

export const i18n = new I18nController();

export function t(
  key: string,
  params?: Record<string, string | number>
): string {
  return i18n.translate(key, params);
}
