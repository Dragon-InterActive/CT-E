# AGENTS.md – Hinweise für KI-Assistenten

Projektziel und Vorgaben: `CLAUDE.md`. Handbuch: `docs/de/README.md` / `docs/en/README.md`.
Ausführliche Anleitung: `.claude/skills/ct-editor/SKILL.md`. Für die UI-Bibliothek gelten
zusätzlich die RUF-Skills unter `../RUF/.claude/skills/` und `../RUF/AGENTS.md`.

## Aufbau

- `src/manifest.rs`, `features.rs`, `crates_io.rs`, `cargo_cli.rs`, `i18n.rs`, `cli.rs` – **ohne Oberfläche**,
  mit Unit-Tests. Logik gehört hierher, nicht in `ui/`.
- `src/ui/mod.rs` – `Editor`, `Shared` (gemeinsamer Zustand), Speichern/Laden/Prüfen.
- `src/ui/*_tab.rs` – je Tab: `build()` (Widgets), `layout(w, h)`, `refresh(&State)`, `connect()`.
- `locales/{de,en}.toml` – alle sichtbaren Texte. `tests/gui.rs` – GUI-Tests (Linux ohne Bildschirm: `xvfb-run -a`). Release für Windows/macOS: `../RUF/tools/bundle-*.sh` (`../RUF/docs/de/release.md`).

## Befehle

```bash
cargo clippy --all-targets          # muss warnungsfrei sein
cargo test                          # Unit- + GUI-Tests, offline
cargo test -- --ignored             # zusätzlich live gegen crates.io
./target/debug/ct-editor --offline --path <ordner> --tab dependencies --screenshot /tmp/ct.png
```

## Regeln

- Sichtbare Texte nur über `tr!("bereich.schluessel")`; neue Schlüssel in **beiden**
  `locales/*.toml` (Tests `languages_are_complete`, `every_used_key_exists`).
- Manifest nur über `Shared::modify` / `try_modify` ändern; danach den eigenen Tab per
  `s.quietly(|st| tab.refresh(st))` befüllen. Programmatisches Befüllen löst Signale
  aus – Handler mit `weak1` registrieren (ignoriert sie während `is_refreshing`).
- Closures halten `Weak<Shared>` (Helfer `weak0`/`weak1`), nie `Rc<Shared>` – sonst Zyklus.
- Netzwerk/Prozesse nie im GUI-Thread: `std::thread` + `ruf::GuiSender`, veraltete
  Antworten per Generationszähler verwerfen.
- Formatierung des Anwenders erhalten: `toml_edit`-Decor bewahren, Tests in
  `manifest.rs` prüfen Round-Trips.
- Keine modalen Dialoge (`ruf::dialog`) in Code, den Tests auslösen.
- Code-Kommentare und Fehlermeldungen auf Deutsch; Doku-Änderungen in **DE und EN**.
- Fehlt in RUF etwas, dort ergänzen (Skills `ruf-bridge`, `rncl-component`), nicht im Editor umgehen.
