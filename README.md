# CT-Editor – Cargo TOML Editor

Visueller Editor für `Cargo.toml`-Dateien – Teil des RAD-Tools **Freya TDE**, gebaut
mit [RUF/RNCL](https://github.com/Dragon-InterActive/RUF) – GTK 4 auf Linux, Windows und macOS.
*Visual editor for `Cargo.toml` files – part of the RAD tool **Freya TDE**, built with
[RUF/RNCL](https://github.com/Dragon-InterActive/RUF) – GTK 4 on Linux, Windows and macOS.*

Eigenständig nutzbar (mit Menü „Datei“) oder als Teil von Freya TDE, das ihn als
Submodul einbindet. Repository: <https://github.com/Dragon-InterActive/CT-E>.
*Usable on its own (with a File menu) or as part of Freya TDE, which includes it as a
submodule.*

![CT-Editor](docs/images/dependencies-de.png)

- **Quelltext** mit Syntax-Hervorhebung und Live-Prüfung · *source with highlighting and live validation*
- **Abhängigkeiten:** Suche auf crates.io (ab 3 Zeichen), Features per Checkbox, Default = Default-Features des Crates · *crates.io search (3+ chars), features as check boxes, preset = the crate's defaults*
- **Paket**, **Features**, **Erweitert** (jeder weitere Abschnitt: `[lib]`, `[[bin]]`, `[profile.*]`, `[workspace]` …) · *full manifest scope*
- formatierungserhaltend (Kommentare bleiben), Prüfung durch `cargo metadata`, Deutsch/Englisch, erweiterbar · *format-preserving, validated by Cargo, i18n*

```bash
ct-editor --lang de --path /pfad/zum/projekt     # Aufruf durch Freya TDE
cargo test                                       # Tests (Linux ohne Bildschirm: xvfb-run -a cargo test)
```

## Dokumentation / Documentation

- 🇩🇪 [Handbuch](docs/de/README.md)
- 🇬🇧 [Manual](docs/en/README.md)
- KI / AI: [`AGENTS.md`](AGENTS.md), [`.claude/skills/ct-editor`](.claude/skills/ct-editor/SKILL.md)

## Lizenz / License

Nach Wahl lizenziert unter / Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

Programme, die damit gebaut werden, dürfen unter beliebiger Lizenz stehen – auch
proprietär. Beiträge werden, sofern nicht ausdrücklich anders angegeben, unter
denselben beiden Lizenzen eingebracht.
*Software built with it may use any license, including proprietary ones. Unless
explicitly stated otherwise, contributions are dual-licensed as above.*
