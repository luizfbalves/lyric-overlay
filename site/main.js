// Demo: repete a mecânica do overlay — a linha atual centralizada e destacada.
// Letras de exemplo inventadas, uma por idioma do site.
const DEMO = {
  pt: [
    "Acendo a luz da cozinha",
    "o rádio ainda sabe o teu nome",
    "a cidade inteira fica quieta",
    "só pra ouvir o refrão passar",
    "e eu canto baixo, sem saber",
    "até a letra aparecer pra mim",
  ],
  en: [
    "I leave the kitchen light on",
    "the radio still knows your name",
    "the whole town goes quiet",
    "just to hear the chorus pass",
    "and I sing low, not knowing",
    "until the words show up for me",
  ],
};
const LINES = document.documentElement.lang.startsWith("pt") ? DEMO.pt : DEMO.en;

const track = document.getElementById("demo-track");
const screen = track.parentElement.parentElement;
const els = LINES.map((t) => {
  const el = document.createElement("span");
  el.className = "demo-line";
  el.textContent = t;
  track.append(el);
  return el;
});

let i = 0;
function show() {
  els.forEach((el, k) => el.classList.toggle("cur", k === i));
  const el = els[i];
  const ty = screen.clientHeight / 2 - (el.offsetTop + el.offsetHeight / 2);
  track.style.transform = `translateY(${ty}px)`;
}
show();
document.fonts?.ready.then(show);
new ResizeObserver(show).observe(screen);

const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
if (!reduced) {
  setInterval(() => {
    i = (i + 1) % LINES.length;
    show();
  }, 2400);
}

// Downloads: aponta os botões direto para os instaladores da última release.
const REPO = "luizfbalves/verso";
const PICK = {
  "mac-arm": (n) => /aarch64\.dmg$/i.test(n),
  "mac-intel": (n) => /x64\.dmg$/i.test(n),
  win: (n) => /x64-setup\.exe$/i.test(n) || /\.msi$/i.test(n),
};

fetch(`https://api.github.com/repos/${REPO}/releases/latest`)
  .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
  .then((rel) => {
    const v = rel.tag_name?.replace(/^v/, "");
    if (v) document.querySelectorAll("[data-version]").forEach((el) => (el.textContent = v));
    document.querySelectorAll("[data-dl]").forEach((a) => {
      const asset = rel.assets.find((x) => PICK[a.dataset.dl](x.name));
      if (asset) a.href = asset.browser_download_url;
    });
  })
  .catch(() => {
    // Sem release publicada ou API indisponível: os links já apontam para a página de Releases.
  });
