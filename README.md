# Verso

App gratuito para macOS e Windows que mostra a letra sincronizada da música que toca no Spotify, flutuando sobre qualquer janela.

Site e downloads: https://luizfbalves.github.io/verso/ (English: https://luizfbalves.github.io/verso/en/)

## Desenvolvimento

Requer Node 22 e Rust estável.

```sh
npm ci
npm run tauri dev     # roda o app em modo dev
npm run tauri build   # gera os instaladores em src-tauri/target/release/bundle
```

Releases saem pelo workflow [release.yml](.github/workflows/release.yml) ao publicar uma tag `v*`.

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

Only the Windows installers (`.exe` and `.msi`) attached to [GitHub Releases](https://github.com/luizfbalves/verso/releases) are signed. They are built from this repository by GitHub Actions and signed only after manual approval of each release.

Team roles:

- Committers and reviewers: [@luizfbalves](https://github.com/luizfbalves)
- Approvers: [@luizfbalves](https://github.com/luizfbalves)

## Privacy policy

Full version: https://luizfbalves.github.io/verso/en/privacy.html (Português: https://luizfbalves.github.io/verso/privacidade.html)

This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

To do its job, Verso sends the following requests:

- **LRCLIB** (https://lrclib.net): the title, artist, album and duration of the track playing in Spotify, to fetch synced lyrics.
- **DeepL** (https://www.deepl.com): only if the user enables translation and enters their own API key. The lyric lines are sent to be translated.

No analytics or telemetry is collected.

## Licença

[MIT](LICENSE)
