export type FontId = "system" | "rounded" | "serif" | "mono" | "handwritten";
export type Mode = "original" | "translated" | "both";
export type TargetLang = "PT-BR" | "EN-US" | "ES";
export type DeepLStatus = "ok" | "invalid_key" | "quota_exceeded";

export interface Appearance {
  font: FontId;
  size: number;
  text_color: string;
  bg_color: string | null;
  bg_opacity: number;
}

export interface TranslationCfg {
  mode: Mode;
  target_lang: TargetLang;
}

export interface Snapshot {
  lines: string[];
  translation: string[] | null;
  index: number;
  visible: boolean;
}

export interface OverlayInit {
  appearance: Appearance;
  mode: Mode;
  snapshot: Snapshot;
  edit: boolean;
}

export interface SettingsView {
  appearance: Appearance;
  translation: TranslationCfg;
  has_key: boolean;
  deepl_status: DeepLStatus;
}

export interface Usage {
  character_count: number;
  character_limit: number;
}

export const DEFAULT_APPEARANCE: Appearance = {
  font: "system",
  size: 1,
  text_color: "#ffffff",
  bg_color: null,
  bg_opacity: 60,
};
