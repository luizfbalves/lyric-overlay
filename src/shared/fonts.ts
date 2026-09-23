import "@fontsource/nunito/700.css";
import "@fontsource/lora/600.css";
import "@fontsource/jetbrains-mono/600.css";
import "@fontsource/caveat/600.css";
import "@fontsource/yomogi/400.css";
import type { FontId } from "./types";

export const FONT_STACKS: Record<FontId, string> = {
  system: '-apple-system,"SF Pro Display","Segoe UI",system-ui,sans-serif',
  rounded: '"Nunito",ui-rounded,system-ui,sans-serif',
  serif: '"Lora",Georgia,serif',
  mono: '"JetBrains Mono",ui-monospace,Consolas,monospace',
  handwritten: '"Caveat","Yomogi",cursive',
};

export const FONT_LABELS: Record<FontId, string> = {
  system: "Sistema",
  rounded: "Arredondada",
  serif: "Serifada",
  mono: "Mono",
  handwritten: "Manuscrita",
};

export function hexToRgba(hex: string, opacity: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${opacity / 100})`;
}
