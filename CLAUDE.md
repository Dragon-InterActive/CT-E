# CT-Editor – Hinweise für KI-Assistenten und Mitwirkende

*Notes for AI assistants and contributors: what the CT-Editor is, its requirements and
conventions. Structure, rules and commands: `AGENTS.md`; manual: `docs/de`, `docs/en`.*

## Was der CT-Editor ist

Der **CT-Editor** (Cargo TOML Editor) erstellt und bearbeitet `Cargo.toml`-Dateien mit
einem visuellen Editor. Er ist eigenständig nutzbar und zugleich Teil des RAD-Tools
[Freya TDE](https://github.com/Dragon-InterActive/Freya-TDE).

- Er stützt sich auf Cargos eigene Mittel (`cargo metadata` zur Prüfung) und überwacht
  die TOML-Syntax; die Formatierung des Anwenders bleibt erhalten (`toml_edit`).
- Neue Crates werden über ein Suchfeld auf crates.io gesucht und ausgewählt; gewählte
  Crates erscheinen mit Kurzbeschreibung und ihren Features, die per Checkbox gewählt
  werden (Standard: die Default-Features).
- Registerkarten: der Quelltext der `Cargo.toml` und die visuelle Schicht
  (Abhängigkeiten, Paket, Features, Erweitert).

## Anforderungen

- **Mehrsprachig (i18n):** alle Texte in `locales/<sprache>.toml`.
- **Zwei Modi aus einer Codebasis:** eigenständig mit Menü „Datei“ (Öffnen, Sprache,
  Beenden; die Sprache wird gespeichert) oder eingebettet (`--embedded`): Freya TDE
  übergibt Pfad und Sprache beim Aufruf, der Editor zeigt kein Menü.
- **Oberfläche mit [RUF](https://github.com/Dragon-InterActive/RUF)** (GTK 4), als
  Git-Abhängigkeit; Fehlendes wird in RUF ergänzt, nicht im Editor umgangen. Die
  RUF-Skills (`.claude/skills/` im RUF-Repository) gelten auch hier.
- Weitere Crates nur in aktuellen, stabilen Versionen und mit freier Lizenz.

## Konventionen

- Rust ≥ 1.99, Edition 2024; `cargo clippy` warnungsfrei.
- Dokumentation mit Beispielen auf Deutsch und Englisch.
- Die Nutzung durch KI-Assistenten mitdenken: Logik ohne Oberfläche als Bibliothek,
  Skill in `.claude/skills/ct-editor/`.
- Lizenz: MIT OR Apache-2.0 (nach Wahl des Nutzers).
