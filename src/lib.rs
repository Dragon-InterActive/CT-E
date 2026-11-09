//! # CT-Editor – Cargo-TOML-Editor für Freya TDE
//!
//! Visueller Editor für `Cargo.toml`, gebaut mit [RUF/RNCL](rncl).
//!
//! | Modul | Aufgabe | Oberfläche nötig |
//! |---|---|---|
//! | [`manifest`] | `Cargo.toml` lesen, ändern, schreiben (formatierungserhaltend) | nein |
//! | [`features`] | Feature-Auswahl ⇄ `features` / `default-features` | nein |
//! | [`crates_io`] | Suche und Crate-Details von crates.io | nein |
//! | [`cargo_cli`] | Prüfung des Manifests durch `cargo metadata` | nein |
//! | [`i18n`] | Übersetzungen (`tr!`) | nein |
//! | [`cli`] | Kommandozeile (Aufruf durch Freya TDE) | nein |
//! | [`settings`] | persönliche Einstellungen (Sprache) des eigenständigen Editors | nein |
//! | [`ui`] | die Oberfläche | ja |
//!
//! Die Logik-Module sind ohne Oberfläche nutzbar – etwa um aus Freya TDE
//! heraus direkt eine Abhängigkeit einzutragen:
//!
//! ```
//! use ct_editor::manifest::{DepKind, DepTable, Dependency, Manifest};
//!
//! let mut manifest = Manifest::new_package("demo");
//! let mut serde = Dependency::new("serde", "1.0.229");
//! serde.features.push("derive".into());
//! manifest.set_dependency(&DepTable::new(DepKind::Normal), &serde);
//! assert!(manifest.to_text().contains(r#"serde = { version = "1.0.229", features = ["derive"] }"#));
//! ```

pub mod cargo_cli;
pub mod cli;
pub mod crates_io;
pub mod features;
pub mod i18n;
pub mod manifest;
pub mod settings;
pub mod ui;
