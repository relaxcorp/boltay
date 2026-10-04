import { api, type AppInfo, type Settings } from "./api";
import { locale } from "./i18n.svelte";

export type Section = "general" | "models" | "text" | "history" | "transcribe" | "about";

/// The settings as the window sees them. Every change is saved right away, there is no
/// "Apply" button: the app behaves like a system utility.
class Store {
  section = $state<Section>("general");
  settings = $state<Settings | null>(null);
  info = $state<AppInfo | null>(null);
  error = $state("");
  savedAt = $state(0);
  #timer: ReturnType<typeof setTimeout> | undefined;

  async load() {
    [this.settings, this.info] = await Promise.all([api.settings(), api.info()]);
    locale.lang = await api.uiLanguage();
  }

  /// Text fields call this on every keystroke, so writes are batched.
  save(delay = 300) {
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.#write(), delay);
  }

  /// Saves at once and hands the error back, for a field that shows it next to itself.
  saveNow(): Promise<string> {
    clearTimeout(this.#timer);
    return this.#write(false);
  }

  async #write(report = true): Promise<string> {
    if (!this.settings) return "";
    try {
      await api.save($state.snapshot(this.settings) as Settings);
      this.error = "";
      this.savedAt = Date.now();
      locale.lang = await api.uiLanguage();
      return "";
    } catch (e) {
      const error = String(e);
      if (report) this.error = error;
      // The backend rejected it: show what is actually in effect.
      this.settings = await api.settings();
      return error;
    }
  }
}

export const store = new Store();
