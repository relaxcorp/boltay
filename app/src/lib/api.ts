import { invoke } from "@tauri-apps/api/core";
import { locale } from "./i18n.svelte";

export type HotkeyMode = "hold" | "toggle";
export type Speech = "russian" | "other";
export type UiLanguage = "en" | "ru";
export type PasteMethod = "clipboard" | "type";
export type OverlayPosition = "bottom" | "top" | "cursor";
export const SOUND_SETS = ["soft", "drop", "click", "sharp", "bell", "shell"] as const;
export type SoundSet = (typeof SOUND_SETS)[number];
export type Profanity = "keep" | "mask" | "remove" | "soften";

export interface Term {
  name: string;
  variants: string[];
}

export interface Terms {
  enabled: boolean;
  entries: Term[];
  seen: string[];
  seen_variants: [string, string][];
}

export interface TextConfig {
  whitespace: boolean;
  fillers: { enabled: boolean; hard: boolean; soft: string[] };
  commands: { enabled: boolean; spoken_punctuation: boolean };
  profanity: Profanity;
  dictionary: { enabled: boolean; entries: { from: string; to: string }[] };
  brands: Terms;
  ambiguous: Terms;
  snippets: {
    enabled: boolean;
    threshold: number;
    entries: { trigger: string; text: string }[];
  };
  cleanup: { enabled: boolean; trailing_space: boolean };
}

export interface Settings {
  hotkey: string;
  translate_hotkey: string;
  translator_hotkey: string;
  quality_translation: boolean;
  mode: HotkeyMode;
  latch: boolean;
  speech: Speech;
  microphone: string | null;
  mic_gain_db: number;
  vad_threshold: number;
  overlay_position: OverlayPosition;
  sounds: boolean;
  sound_set: SoundSet;
  autostart: boolean;
  ui_language: UiLanguage | null;
  paste: PasteMethod;
  restore_clipboard: boolean;
  paragraph_messages: boolean;
  check_updates: boolean;
  paragraph_pause_secs: number | null;
  max_recording_secs: number;
  unload_after_minutes: number | null;
  graph_cache: boolean;
  models_dir: string | null;
  history: { enabled: boolean; retention_days: number | null };
  text: TextConfig;
}

export type ModelId = "gigaam-v3-e2e-rnnt" | "parakeet-v3" | "silero-vad" | "opus-mt-ru-en"
  | "opus-mt-tc-big-zle-en" | "opus-mt-en-ru" | "opus-mt-tc-big-en-zle";

export interface ModelStatus {
  id: ModelId;
  size: number;
  done: number;
  ready: boolean;
}

export interface Models {
  dir: string;
  default_dir: string;
  items: ModelStatus[];
  downloading: ModelId[];
  loaded: boolean;
  loading: boolean;
}

export const SPEECH_MODEL: Record<Speech, ModelId> = {
  russian: "gigaam-v3-e2e-rnnt",
  other: "parakeet-v3",
};

/// What must be on disk to dictate in a language.
export const needed = (speech: Speech): ModelId[] => [SPEECH_MODEL[speech], "silero-vad"];

/// Binary megabytes, as file managers on Windows show them.
export function megabytes(bytes: number): string {
  const mb = bytes / 1024 / 1024;
  const [megs, gigs] = locale.lang === "ru" ? ["МБ", "ГБ"] : ["MB", "GB"];
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} ${gigs}` : `${Math.max(1, Math.round(mb))} ${megs}`;
}

export interface Entry {
  at: number;
  text: string;
  raw: string;
  duration_ms: number;
}

export interface AppInfo {
  version: string;
  os: string;
  display: "x11" | "wayland" | "native";
  soft_fillers: string[];
  default_brands: Term[];
  config_dir: string;
  data_dir: string;
  first_run: boolean;
}

export type Access = "granted" | "denied" | "unknown";

export interface Permissions {
  needed: boolean;
  microphone: Access;
  accessibility: Access;
}

/// The response code a failed download ended with, for a short message; the rest goes
/// under "Details".
export function httpStatus(error: string): string | null {
  return error.match(/HTTP (\d{3})/)?.[1] ?? null;
}

/// One failed source per line.
export const detailLines = (error: string) => error.replaceAll("; ", "\n");

export interface Release {
  version: string;
  url: string;
  notes: string;
}

export interface UpdateStatus {
  checking: boolean;
  checked: boolean;
  available: Release | null;
}

export interface Delta {
  added: number;
  removed: number;
}

export interface ImportPreview {
  file: string;
  modified: number | null;
  changes: number;
  dictionary: Delta;
  snippets: Delta;
  brands: Delta;
}

export interface Heard {
  raw: string;
  text: string;
}

export type Direction = "ru_en" | "en_ru";
export type Mark = "low_confidence" | "rare" | "abbreviation" | "slang";

export interface Piece {
  text: string;
  mark: Mark | null;
  note: string | null;
}

export interface Chip {
  term: string;
  meaning: string;
}

export type Outcome =
  | {
      state: "done";
      direction: Direction;
      translation: Piece[];
      original: Piece[];
      chips: Chip[];
      /// The big model downloading while the base one translates.
      upgrading: ModelId | null;
      /// Only the start of a longer text was translated.
      cut: boolean;
    }
  | { state: "need_model"; direction: Direction; base: number; big: number }
  | { state: "fetching"; direction: Direction; id: ModelId }
  | { state: "empty" };

/// What the translator was opened on; `text` is null when nothing was selected.
export interface Opened {
  seq: number;
  text: string | null;
}

export interface TranscribeJob {
  file: string | null;
  running: boolean;
  progress: number;
  result: { text: string; language: "ru" | "en"; seconds: number } | null;
  error: string | null;
}

export const plain = (pieces: Piece[]) => pieces.map((p) => p.text).join("");

export const api = {
  settings: () => invoke<Settings>("get_settings"),
  save: (settings: Settings) => invoke<void>("save_settings", { settings }),
  uiLanguage: () => invoke<UiLanguage>("ui_language"),
  suspendHotkey: (suspended: boolean) => invoke<void>("suspend_hotkey", { suspended }),
  hotkeyError: () => invoke<string | null>("hotkey_error"),
  microphones: () => invoke<string[]>("microphones"),
  models: () => invoke<Models>("models"),
  download: (id: ModelId) => invoke<void>("download_model", { id }),
  cancelDownload: (id: ModelId) => invoke<void>("cancel_download", { id }),
  deleteModel: (id: ModelId) => invoke<void>("delete_model", { id }),
  history: () => invoke<Entry[]>("history"),
  deleteHistory: (at: number) => invoke<void>("delete_history", { at }),
  clearHistory: () => invoke<void>("clear_history"),
  copy: (text: string) => invoke<void>("copy_text", { text }),
  copyAndDismiss: (text: string) => invoke<void>("copy_and_dismiss", { text }),
  dismissOverlay: () => invoke<void>("dismiss_overlay"),
  expandOverlay: () => invoke<void>("expand_overlay"),
  info: () => invoke<AppInfo>("app_info"),
  permissions: () => invoke<Permissions>("permissions"),
  requestMicrophone: () => invoke<void>("request_microphone"),
  requestAccessibility: () => invoke<void>("request_accessibility"),
  openPermissionSettings: (kind: "microphone" | "accessibility") =>
    invoke<void>("open_permission_settings", { kind }),
  openLink: (url: string) => invoke<void>("open_link", { url }),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  openLogs: () => invoke<void>("open_logs"),
  openNotices: () => invoke<void>("open_notices"),
  preview: (config: TextConfig, text: string) => invoke<string>("preview_text", { config, text }),
  closeOnboarding: () => invoke<void>("close_onboarding"),
  previewSound: (set: SoundSet) => invoke<void>("preview_sound", { set }),
  micMonitor: (on: boolean) => invoke<void>("mic_monitor", { on }),
  micVolume: () => invoke<number | null>("mic_volume"),
  raiseMicVolume: () => invoke<void>("raise_mic_volume"),
  updateStatus: () => invoke<UpdateStatus>("update_status"),
  checkUpdates: () => invoke<UpdateStatus>("check_updates_now"),
  exportSettings: () => invoke<string | null>("export_settings"),
  importSettings: () => invoke<ImportPreview | null>("import_settings"),
  applyImport: () => invoke<void>("apply_import"),
  cancelImport: () => invoke<void>("cancel_import"),
  resetSettings: () => invoke<void>("reset_settings"),
  voiceCheckStart: () => invoke<void>("voice_check_start"),
  voiceCheckStop: () => invoke<Heard>("voice_check_stop"),
  voiceCheckCancel: () => invoke<void>("voice_check_cancel"),
  translatorOpened: () => invoke<Opened | null>("translator_opened"),
  translate: (text: string, direction: Direction | null = null) =>
    invoke<Outcome>("translate_text", { text, direction }),
  translatorFit: (height: number) => invoke<void>("translator_fit", { height }),
  translatorClose: () => invoke<void>("translator_close"),
  translatorMoved: () => invoke<void>("translator_moved"),
  translatorKeyboard: () => invoke<void>("translator_keyboard"),
  translatorInsert: (text: string) => invoke<void>("translator_insert", { text }),
  translatorFetch: (direction: Direction, quality: boolean) =>
    invoke<void>("translator_fetch", { direction, quality }),
  transcribeJob: () => invoke<TranscribeJob>("transcribe_job"),
  transcribePick: () => invoke<boolean>("transcribe_pick"),
  transcribePath: (path: string) => invoke<void>("transcribe_path", { path }),
  transcribeCancel: () => invoke<void>("transcribe_cancel"),
};
