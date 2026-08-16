// Theme store, ported from Alexandria's `theme.svelte.ts` so all three apps
// behave identically: preference is light | dark | system, persisted under
// "theme", and `.dark` is toggled on <html>.
//
// The one difference: lgtm defaults to LIGHT rather than system. It's a reading
// tool, and light is the surface you read on.

export type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "theme";
const ORDER: Theme[] = ["light", "dark", "system"];

export const THEME_LABEL: Record<Theme, string> = {
  light: "☀︎ Light",
  dark: "☾ Dark",
  system: "◐ System",
};

function systemPrefersDark(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function readStored(): Theme {
  if (typeof localStorage === "undefined") return "light";
  const v = localStorage.getItem(STORAGE_KEY);
  return v === "light" || v === "dark" || v === "system" ? v : "light";
}

class ThemeStore {
  preference = $state<Theme>("light");
  /** What is actually applied right now. */
  resolved = $state<"light" | "dark">("light");

  init() {
    if (typeof document === "undefined") return;
    this.preference = readStored();
    this.apply();

    // Follow the system only while the preference says to.
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", () => {
      if (this.preference === "system") this.apply();
    });
  }

  set(next: Theme) {
    this.preference = next;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(STORAGE_KEY, next);
    }
    this.apply();
  }

  cycle() {
    const idx = ORDER.indexOf(this.preference);
    this.set(ORDER[(idx + 1) % ORDER.length]);
  }

  get label(): string {
    return THEME_LABEL[this.preference];
  }

  private apply() {
    if (typeof document === "undefined") return;
    const isDark =
      this.preference === "dark" ||
      (this.preference === "system" && systemPrefersDark());
    document.documentElement.classList.toggle("dark", isDark);
    this.resolved = isDark ? "dark" : "light";
  }
}

export const theme = new ThemeStore();
