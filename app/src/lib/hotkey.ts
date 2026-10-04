import { locale } from "./i18n.svelte";

const mac = navigator.userAgent.includes("Mac");
const ru = () => locale.lang === "ru";

/// A single key watched by the input hooks rather than registered with the system.
export const isSolo = (hotkey: string) => /^(ControlRight|AltRight|ShiftRight|CapsLock)$/.test(hotkey);

/// `KeyA` -> `A`, `Digit1` -> `1`, `Super` -> `Win` or `Cmd`, `ControlRight` -> `Right Ctrl`:
/// the stored form is what the hotkey parser reads, this one is for people.
export function pretty(hotkey: string): string {
  return hotkey
    .split("+")
    .map((part) =>
      part
        .replace(/^Key([A-Z])$/, "$1")
        .replace(/^Digit(\d)$/, "$1")
        .replace(/^Super$/, mac ? "Cmd" : "Win")
        .replace(/^Alt$/, mac ? "Option" : "Alt")
        .replace(/^ControlRight$/, ru() ? "Правый Ctrl" : "Right Ctrl")
        .replace(/^AltRight$/, ru() ? "Правый Alt" : "Right Alt")
        .replace(/^ShiftRight$/, ru() ? "Правый Shift" : "Right Shift"),
    )
    .join(" + ");
}
