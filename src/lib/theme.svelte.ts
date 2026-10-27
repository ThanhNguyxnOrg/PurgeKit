export type ThemeMode = "auto" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "purgekit.theme";
const COLOR_BLIND_KEY = "purgekit.colorblind";

class ThemeController {
  mode = $state<ThemeMode>("auto");
  colorBlind = $state(false);
  systemLight = $state(false);
  resolved: ResolvedTheme = $derived(
    this.mode === "auto" ? (this.systemLight ? "light" : "dark") : this.mode
  );

  private media: MediaQueryList | null = null;

  init() {
    if (this.media) return;
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved === "auto" || saved === "light" || saved === "dark") {
        this.mode = saved;
      }
      this.colorBlind = localStorage.getItem(COLOR_BLIND_KEY) === "1";
    } catch {}
    if (typeof window !== "undefined" && window.matchMedia) {
      this.media = window.matchMedia("(prefers-color-scheme: light)");
      this.systemLight = this.media.matches;
      this.media.addEventListener("change", this.handleSystemChange);
    }
    this.apply();
  }

  setMode(mode: ThemeMode) {
    this.mode = mode;
    try {
      localStorage.setItem(STORAGE_KEY, mode);
    } catch {}
    this.apply();
  }

  setColorBlind(enabled: boolean) {
    this.colorBlind = enabled;
    try {
      localStorage.setItem(COLOR_BLIND_KEY, enabled ? "1" : "0");
    } catch {}
    this.apply();
  }

  destroy() {
    this.media?.removeEventListener("change", this.handleSystemChange);
    this.media = null;
  }

  private handleSystemChange = (event: MediaQueryListEvent) => {
    this.systemLight = event.matches;
    if (this.mode === "auto") {
      this.apply();
    }
  };

  private apply() {
    if (typeof document === "undefined") return;
    const resolved = this.resolved;
    document.documentElement.dataset.theme = resolved;
    document.documentElement.dataset.colorblind = this.colorBlind ? "true" : "false";
    document.documentElement.style.colorScheme = resolved;
    void syncWindowTheme(resolved);
  }
}

async function syncWindowTheme(theme: ResolvedTheme) {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setTheme(theme);
  } catch {}
}

export const theme = new ThemeController();
