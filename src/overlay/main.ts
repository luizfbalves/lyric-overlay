import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { FONT_STACKS, hexToRgba } from "../shared/fonts";
import type { Appearance, Mode, OverlayInit } from "../shared/types";
import "./style.css";

const root = document.getElementById("overlay")!;
const viewport = document.getElementById("viewport")!;
const track = document.getElementById("track")!;
const toast = document.getElementById("toast")!;

// Texto de exemplo do modo de edição (inventado).
const SAMPLE = ["Letra de exemplo", "Arraste para posicionar", "Cmd/Ctrl+Shift+L para concluir"];
const MODE_CLASS: Record<Mode, string> = { original: "m-orig", translated: "m-tr", both: "m-both" };

let lines: string[] = [];
let translation: string[] | null = null;
let index = -1;
let mode: Mode = "both";
let visible = false;
let editing = false;
let toastTimer: number | undefined;

function applyMode() {
  root.classList.remove("m-orig", "m-tr", "m-both");
  root.classList.add(translation ? MODE_CLASS[mode] : "m-orig");
}

function render() {
  const src = lines.length ? lines : editing ? SAMPLE : [];
  const nodes = src.map((text, i) => {
    const ln = document.createElement("div");
    ln.className = "ln";
    if (!text.trim()) {
      ln.classList.add("gap");
      ln.textContent = "• • •";
      return ln;
    }
    const tr = translation?.[i] ?? "";
    if (!tr) ln.classList.add("no-tr");
    const t = document.createElement("span");
    t.className = "t";
    t.textContent = tr || text;
    const o = document.createElement("span");
    o.className = "o";
    o.textContent = text;
    ln.append(t, o);
    return ln;
  });
  track.replaceChildren(...nodes);
  applyMode();
  focus();
}

function focus() {
  const els = track.children;
  const cur = lines.length ? index : editing ? 1 : -1;
  for (let i = 0; i < els.length; i++) els[i].classList.toggle("cur", i === cur);
  const el = els[Math.max(cur, 0)] as HTMLElement | undefined;
  if (!el) {
    track.style.transform = "";
    return;
  }
  const ty = viewport.clientHeight / 2 - (el.offsetTop + el.offsetHeight / 2);
  track.style.transform = `translateY(${ty}px)`;
}

function updateVisibility() {
  root.classList.toggle("hidden", !visible && !editing);
}

function applyAppearance(a: Appearance) {
  const s = root.style;
  s.setProperty("--ov-font", FONT_STACKS[a.font] ?? FONT_STACKS.system);
  s.setProperty("--ov-color", a.text_color);
  s.setProperty("--ov-scale", String(a.size));
  s.setProperty("--ov-bg", a.bg_color ? hexToRgba(a.bg_color, a.bg_opacity) : "transparent");
  root.classList.toggle("has-bg", !!a.bg_color);
  requestAnimationFrame(focus);
}

function showToast(text: string) {
  toast.textContent = text;
  toast.classList.add("show");
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toast.classList.remove("show"), 1000);
}

root.addEventListener("mousedown", (e) => {
  if (editing && e.button === 0) void getCurrentWindow().startDragging();
});

new ResizeObserver(() => focus()).observe(viewport);
document.fonts?.ready.then(() => focus());

async function main() {
  await listen<{ lines: string[] }>("lyrics-loaded", (e) => {
    lines = e.payload.lines;
    translation = null;
    index = -1;
    render();
  });
  await listen<{ lines: string[] }>("translation-loaded", (e) => {
    translation = e.payload.lines.length ? e.payload.lines : null;
    render();
  });
  await listen<{ index: number }>("line-changed", (e) => {
    index = e.payload.index;
    focus();
  });
  await listen("hide", () => {
    visible = false;
    updateVisibility();
  });
  await listen("show", () => {
    visible = true;
    updateVisibility();
  });
  await listen<Appearance>("appearance-changed", (e) => applyAppearance(e.payload));
  await listen<{ mode: Mode }>("mode-changed", (e) => {
    mode = e.payload.mode;
    applyMode();
    requestAnimationFrame(focus);
  });
  await listen<{ offset_ms: number }>("offset-changed", (e) => {
    const v = e.payload.offset_ms;
    showToast(`offset ${v > 0 ? "+" : ""}${v} ms`);
  });
  await listen<{ on: boolean }>("edit-mode", (e) => {
    editing = e.payload.on;
    root.classList.toggle("edit", editing);
    render();
    updateVisibility();
  });

  const init = await invoke<OverlayInit>("get_overlay_init");
  applyAppearance(init.appearance);
  mode = init.mode;
  lines = init.snapshot.lines;
  translation = init.snapshot.translation;
  index = init.snapshot.index;
  visible = init.snapshot.visible;
  editing = init.edit;
  root.classList.toggle("edit", editing);
  render();
  updateVisibility();
}

void main();
