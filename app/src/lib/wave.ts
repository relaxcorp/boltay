export type Theme =
  | "recording"
  | "recognition"
  | "commands"
  | "fillers"
  | "profanity"
  | "brands"
  | "check"
  | "models"
  | "dictionary"
  | "snippets"
  | "formatting"
  | "system"
  | "models-folder"
  | "licenses"
  | "backup"
  | "advanced";

type Kind =
  | "voice"
  | "totext"
  | "commands"
  | "stutter"
  | "bleep"
  | "ping"
  | "ecg"
  | "crescendo"
  | "ticks"
  | "repeat"
  | "paragraphs"
  | "blip"
  | "double"
  | "calm";

// The louder the line, the more important the section.
const THEMES: Record<Theme, [Kind, 1 | 2 | 3]> = {
  recording: ["voice", 3],
  recognition: ["totext", 3],
  commands: ["commands", 2],
  fillers: ["stutter", 2],
  profanity: ["bleep", 2],
  brands: ["ping", 2],
  check: ["ecg", 2],
  models: ["crescendo", 2],
  dictionary: ["ticks", 1],
  snippets: ["repeat", 1],
  formatting: ["paragraphs", 1],
  system: ["calm", 1],
  "models-folder": ["blip", 1],
  licenses: ["calm", 1],
  backup: ["double", 1],
  advanced: ["blip", 1],
};

export interface Wave {
  d: string;
  level: number;
  /** The "beep" plate laid over the profanity line: x and width. */
  bar?: [number, number];
}

/** Height of the drawing; the axis runs through the middle. */
export const HEIGHT = 15;

export function wave(theme: Theme): Wave {
  const [kind, level] = THEMES[theme];
  const amp = [0, 3, 4.4, 6][level];
  const Y = HEIGHT / 2;
  const p: string[] = [];
  let x = 0;
  const move = (nx: number) => {
    x = nx;
    p.push(`M${x.toFixed(2)} ${Y}`);
  };
  const flat = (dx: number) => {
    x += dx;
    p.push(`L${x.toFixed(2)} ${Y}`);
  };
  const to = (dx: number, dy: number) => {
    x += dx;
    p.push(`L${x.toFixed(2)} ${(Y + dy).toFixed(2)}`);
  };
  // One beat: up, through the axis down, back onto the axis.
  const spike = (h: number, w = 1.4) => {
    to(w, -h);
    to(w * 1.4, h * 0.7);
    to(w, 0);
  };
  const swing = (envelope: number[], step = 2.8) =>
    envelope.forEach((e, i) => to(step, (i % 2 ? 1 : -1) * e * amp));

  move(0);
  let bar: [number, number] | undefined;
  switch (kind) {
    case "voice":
      flat(6);
      swing([0.3, 0.6, 1, 0.7, 0.9, 0.55, 0.8, 0.45, 0.7, 0.35, 0.55, 0.28, 0.42, 0.2, 0.3, 0.12]);
      to(2.8, 0);
      flat(6);
      break;
    case "totext":
      flat(6);
      swing([0.9, 1, 0.7, 0.75, 0.45, 0.45, 0.22, 0.2]);
      to(2.8, 0);
      flat(5);
      for (let i = 0; i < 5; i++) {
        spike(1.8, 0.9);
        flat(4);
      }
      break;
    case "commands":
      for (let i = 0; i < 3; i++) {
        flat(8);
        spike(amp, 1.2);
      }
      flat(8);
      break;
    case "stutter":
      flat(4);
      for (let i = 0; i < 4; i++) {
        to(2.2, -amp * 0.55);
        to(2.2, amp * 0.55);
      }
      to(1.6, 0);
      move(x + 12);
      for (let i = 0; i < 3; i++) {
        to(2.2, -amp * 0.4);
        to(2.2, amp * 0.4);
      }
      to(1.6, 0);
      flat(4);
      break;
    case "bleep":
      flat(24);
      bar = [x + 2, 22];
      move(x + 26);
      flat(24);
      break;
    case "ping":
      flat(14);
      spike(amp * 1.1, 1.6);
      flat(9);
      spike(amp * 0.45);
      flat(9);
      spike(amp * 0.2, 1);
      flat(12);
      break;
    case "ecg":
      for (let i = 0; i < 2; i++) {
        flat(10);
        to(2, -1.4);
        to(2, 0);
        flat(3);
        to(1.2, 1.4);
        to(1.8, -amp * 1.05);
        to(2, amp * 0.65);
        to(1.4, 0);
        flat(5);
        to(3, -1.6);
        to(3, 0);
      }
      flat(8);
      break;
    case "crescendo":
      for (let i = 0; i < 4; i++) {
        flat(8);
        spike(amp * (0.3 + 0.24 * i));
      }
      flat(8);
      break;
    case "ticks":
      flat(4);
      for (let i = 0; i < 8; i++) {
        flat(4.5);
        spike(amp * (i % 3 === 1 ? 0.6 : 0.38), 0.9);
      }
      flat(6);
      break;
    case "repeat":
      for (let i = 0; i < 3; i++) {
        flat(7);
        to(1.6, -amp);
        to(1.8, amp * 0.5);
        to(1.6, -amp * 0.3);
        to(1.4, 0);
      }
      flat(8);
      break;
    case "paragraphs":
      flat(4);
      for (let i = 0; i < 3; i++) {
        for (let j = 0; j < 3; j++) {
          to(2.2, -amp * 0.55);
          to(2.2, amp * 0.4);
        }
        to(1.6, 0);
        if (i < 2) move(x + 7);
      }
      flat(4);
      break;
    case "blip":
      flat(22);
      spike(amp * 0.7, 1.2);
      flat(22);
      break;
    case "double":
      flat(16);
      spike(amp * 0.8, 1.2);
      flat(4);
      spike(amp * 0.8, 1.2);
      flat(16);
      break;
    case "calm":
      flat(16);
      spike(amp * 0.6);
      flat(6);
      spike(amp * 0.35, 1);
      flat(16);
      break;
  }
  return { d: p.join(" "), level, bar };
}
