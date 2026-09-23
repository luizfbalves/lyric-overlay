# DESIGN.md — site do Lyric Overlay (`site/`)

## Contexto
- **Artefato:** landing page de download de um app desktop pessoal, gratuito (macOS/Windows).
- **Público:** quem ouve Spotify no computador e quer a letra na tela.
- **Ação principal:** baixar o instalador do seu sistema.
- **Adjetivos:** editorial, musical, artesanal, direto, calmo.
- **Essência:** encarte de disco.

## Direção
Encarte/contracapa de LP impresso: papel com grão, tinta preta, **uma cor especial** (vermilion) usada como
marca-texto. Faixas de catálogo em mono, fios pretos separando seções, conteúdo organizado em **Lado A**
(recursos, como tracklist) e **Lado B** (instalação).

**Assinatura:** o marca-texto vermilion atrás da linha atual — é a própria mecânica do app (fundo só sob a
linha em destaque), repetida na demo do hero e no hover da tracklist.

## Tipografia
- Display: **Gambetta** (Fontshare) — serifa editorial, itálico no hero.
- Texto: **IBM Plex Sans**. Catálogo, atalhos, legendas: **IBM Plex Mono**.
- Escala 1.333 (quarta justa), base 17px: 12 / 14 / 17 / 22.7 / 30.2 / clamp(41.6–71.7).

## Cor (OKLCH)
| papel | valor | uso |
|---|---|---|
| `--paper` | `oklch(0.945 0.006 100)` | fundo (cinza-papel, não creme) |
| `--paper-2` | `oklch(0.915 0.008 100)` | superfícies, hover |
| `--ink` | `oklch(0.2 0.012 60)` | texto, fios, botão primário |
| `--muted` | `oklch(0.44 0.012 60)` | texto secundário |
| `--spot` | `oklch(0.63 0.19 33)` | marca-texto, numeração dos passos |
| `--focus` | `oklch(0.5 0.19 33)` | anel de foco |

## Regras
- Espaçamento base 8px; apertado dentro de grupos, 64–80px entre seções.
- Raios: 0 (caixas, fios) e pílula (botões, selos "Lado A/B").
- Sem sombras: bordas definidas (fios de 1–2px).
- Botões ranqueados: primário tinta cheia, secundário contorno. Mínimo 48px de altura.
- Movimento: ease-out; demo troca de linha a cada 2,4s; tudo desligado com `prefers-reduced-motion`.
- Sem modo escuro: é papel impresso, de propósito.

## Downloads
`main.js` consulta `releases/latest` do repositório e aponta os botões para o `.dmg` (aarch64/x64) e o
`-setup.exe`. Sem release, os links caem na página de Releases.

## Auditoria anti-slop
Sem Inter/roxo/gradiente/glass/cards com ícone; estrutura hero → tracklist → passos → colofão (não o
boilerplate de 3 cards). Foco visível, alvos ≥ 24px, sem cor como único sinal, sem rolagem horizontal a 375px.

## Changelog
- 2026-09-23 — versão inicial.
