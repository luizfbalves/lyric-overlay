import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import bmcButton from "../assets/bmc/bmc-button.svg";
import { FONT_LABELS, FONT_STACKS } from "../shared/fonts";
import {
  DEFAULT_APPEARANCE,
  type Appearance,
  type FontId,
  type Mode,
  type SettingsView,
  type TargetLang,
  type TranslateStatus,
  type TranslationCfg,
} from "../shared/types";
import "./style.css";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

let appearance: Appearance = { ...DEFAULT_APPEARANCE };
let translation: TranslationCfg = { mode: "original", target_lang: "PT-BR" };
let saveTimer: number | undefined;

function saveAppearance() {
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => void invoke("set_appearance", { appearance }), 120);
}

function setAppearance(patch: Partial<Appearance>) {
  appearance = { ...appearance, ...patch };
  renderAppearance();
  saveAppearance();
}

function buildFonts() {
  const box = $("fonts");
  box.replaceChildren(
    ...(Object.keys(FONT_LABELS) as FontId[]).map((id) => {
      const b = document.createElement("button");
      b.dataset.font = id;
      const aa = document.createElement("b");
      aa.textContent = "Aa";
      aa.style.fontFamily = FONT_STACKS[id];
      const label = document.createElement("small");
      label.textContent = FONT_LABELS[id];
      b.append(aa, label);
      b.onclick = () => setAppearance({ font: id });
      return b;
    }),
  );
}

function renderAppearance() {
  document.querySelectorAll<HTMLButtonElement>("#fonts button").forEach((b) =>
    b.setAttribute("aria-pressed", String(b.dataset.font === appearance.font)),
  );
  document.querySelectorAll<HTMLButtonElement>("#sizes button").forEach((b) =>
    b.setAttribute("aria-pressed", String(Number(b.dataset.size) === appearance.size)),
  );
  document.querySelectorAll<HTMLButtonElement>("#txt-sw button").forEach((b) => {
    b.style.background = b.dataset.c!;
    b.setAttribute("aria-pressed", String(b.dataset.c === appearance.text_color));
  });
  document.querySelectorAll<HTMLButtonElement>("#bg-sw button").forEach((b) => {
    if (b.dataset.c) b.style.background = b.dataset.c;
    b.setAttribute("aria-pressed", String((b.dataset.c || null) === appearance.bg_color));
  });
  $<HTMLInputElement>("txt-pick").value = appearance.text_color;
  if (appearance.bg_color) $<HTMLInputElement>("bg-pick").value = appearance.bg_color;
  const op = $<HTMLInputElement>("bg-op");
  op.value = String(appearance.bg_opacity);
  op.disabled = !appearance.bg_color;
  $("bg-opv").textContent = `${appearance.bg_opacity}%`;
}

function renderStatus(status: TranslateStatus) {
  const warn = $("warn");
  const msg: Record<TranslateStatus, string> = {
    ok: "",
    quota_exceeded: "A cota grátis de tradução deste mês acabou. Mostrando só a letra original até o mês virar.",
  };
  warn.textContent = msg[status];
  warn.hidden = status === "ok";
}

function wire() {
  document.querySelectorAll<HTMLButtonElement>("#sizes button").forEach((b) => {
    b.onclick = () => setAppearance({ size: Number(b.dataset.size) });
  });
  document.querySelectorAll<HTMLButtonElement>("#txt-sw button").forEach((b) => {
    b.onclick = () => setAppearance({ text_color: b.dataset.c! });
  });
  document.querySelectorAll<HTMLButtonElement>("#bg-sw button").forEach((b) => {
    b.onclick = () => setAppearance({ bg_color: b.dataset.c || null });
  });
  $<HTMLInputElement>("txt-pick").oninput = (e) => setAppearance({ text_color: (e.target as HTMLInputElement).value });
  $<HTMLInputElement>("bg-pick").oninput = (e) => setAppearance({ bg_color: (e.target as HTMLInputElement).value });
  $<HTMLInputElement>("bg-op").oninput = (e) => setAppearance({ bg_opacity: Number((e.target as HTMLInputElement).value) });
  $("reset").onclick = () => setAppearance({ ...DEFAULT_APPEARANCE });

  $<HTMLSelectElement>("lang").onchange = (e) => {
    translation = { ...translation, target_lang: (e.target as HTMLSelectElement).value as TargetLang };
    void invoke("set_translation", { translation });
  };

  $("bmc").onclick = () => invoke("open_link", { link: "support" }).catch((err) => console.error(err));
  $<HTMLImageElement>("bmc-img").src = bmcButton;
}

async function main() {
  buildFonts();
  wire();
  await listen<{ mode: Mode; target_lang: TargetLang }>("mode-changed", (e) => {
    translation = { mode: e.payload.mode, target_lang: e.payload.target_lang };
    $<HTMLSelectElement>("lang").value = translation.target_lang;
  });
  await listen<TranslateStatus>("translate-status", (e) => renderStatus(e.payload));

  const s = await invoke<SettingsView>("get_settings");
  appearance = s.appearance;
  translation = s.translation;
  $("translation").hidden = !s.translation_enabled;
  $<HTMLSelectElement>("lang").value = translation.target_lang;
  renderAppearance();
  renderStatus(s.translate_status);
}

void main();
