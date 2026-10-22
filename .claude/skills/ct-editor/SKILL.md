---
name: ct-editor
description: Den CT-Editor (Cargo-TOML-Editor von Freya TD) erweitern oder Fehler darin suchen – neue Tabs, Paketfelder, Manifest-Funktionen, Übersetzungen, crates.io-Anbindung, GUI-Tests und Screenshots. Verwenden bei Arbeiten im Ordner CT-Editor oder wenn Freya TD Cargo.toml-Dateien bearbeiten soll.
---

# CT-Editor erweitern

Erst `AGENTS.md` lesen (Regeln), dann das passende Rezept. UI-Grundlagen: RUF-Skill `ruf-app`.

## Wo gehört was hin?

| Aufgabe | Datei | Test |
|---|---|---|
| Manifest lesen/ändern (neue Abschnitte, Schreibweisen) | `src/manifest.rs` | Unit-Test im selben Modul |
| Feature-Logik | `src/features.rs` | Unit-Test |
| crates.io / andere Registry | `src/crates_io.rs` (`CrateSource`) | Unit-Test + `#[ignore]`-Livetest |
| Text in der Oberfläche | `locales/de.toml` **und** `locales/en.toml` | `cargo test --lib i18n` |
| Widgets, Layout, Bedienung | `src/ui/<tab>.rs` | `tests/gui.rs` |
| Startoptionen | `src/cli.rs` | Unit-Test |

## Rezept: neues `[package]`-Feld

1. Eintrag in `PACKAGE_FIELDS` (`src/manifest.rs`) mit passendem `FieldKind`.
2. Tooltip `package.field.<key>` in beiden `locales/*.toml`.
3. Fertig – Formular, Laden und Speichern sind generisch (`package_tab.rs`).
   Test `package_field_tooltips_exist` prüft die Übersetzung.

## Rezept: neue Abschnittsvorlage (Tab "Erweitert")

`SECTION_TEMPLATES` in `src/manifest.rs` ergänzen: `("schluessel", "[toml]\n…")`.
Der Test `sections` prüft, dass jede Vorlage gültiges TOML ergibt.

## Rezept: neuer Tab

1. `src/ui/neu_tab.rs` nach Vorbild `features_tab.rs`: `pub struct NeuTab { pub page: RnclPanel, … }`
   mit `build(page)`, `layout(w, h)`, `refresh(&State)` und `pub fn connect(shared: &Rc<Shared>)`.
2. In `ui/mod.rs`: `pub mod`, Feld in `Shared`, `NeuTab::build(&tabs.add_tab(&tr!("tab.neu")))`,
   `neu_tab::connect(&shared)`, `Origin::Neu`, Eintrag in `Shared::refresh`.
3. `cli::Tab` um die Variante ergänzen (Reihenfolge = Tab-Index).
4. Texte in beiden Sprachdateien, Dateiliste im i18n-Test `every_used_key_exists` ergänzen.
5. GUI-Test in `tests/gui.rs` (Funktion **und** Eintrag in der `tests`-Liste).

Muster in `connect`:

```rust
tab.button.on_clicked(weak0(shared, |s| {
    s.modify(Origin::Neu, |m| { /* Manifest ändern */ });
    s.quietly(|state| s.neu.refresh(state));          // eigenen Tab nachziehen
    s.set_status(StatusKind::Success, &tr!("neu.done"));
}));
tab.field.on_text_changed(weak1(shared, |s, text: String| { /* … */ }));
```

## Rezept: Hintergrundarbeit (Netzwerk, Prozesse)

Vorbild `deps_tab.rs` (`start_search` / `on_search_reply`): Generationszähler erhöhen,
`std::thread::spawn`, Ergebnis per `GuiSender` (in `connect` angelegt, in einer
`OnceCell` im Tab gespeichert) zurück, im Handler Generation vergleichen.

## Prüfen

```bash
cargo clippy --all-targets && cargo test
./target/debug/ct-editor --offline --lang de --path /tmp/demo --tab <tab> --screenshot /tmp/ct.png
xvfb-run -a cargo run --example remote_control -- /tmp/demo serde derive /tmp/d.png
```

Bild ansehen: Überlappungen, abgeschnittene Texte, leere Bereiche. Bei Layoutproblemen
die `layout(w, h)`-Funktion des Tabs prüfen (absolute Positionen, Mindestgrößen per `clamp`).

## Fallstricke

- Signale feuern auch bei `set_text`/`set_items` → Endlosschleifen oder überschriebene
  Eingaben, wenn ein Handler nicht über `weak1` läuft bzw. nicht `quietly` befüllt wird.
- `RnclComboBox`: Text setzen mit `set_current_text`, nicht `set_text`.
- Während eines Syntaxfehlers im Quelltext ignoriert `modify` alle Änderungen (Absicht).
- Feature-Reihenfolge im Manifest bleibt erhalten (`apply_selection`) – Tests danach ausrichten.
