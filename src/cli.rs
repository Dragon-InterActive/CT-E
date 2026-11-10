//! Kommandozeile – so ruft Freya TDE den Editor auf.
//!
//! ```text
//! ct-editor --lang de --path /pfad/zum/projekt
//! ct-editor -l en ./mein-projekt
//! ```

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// Die Registerkarten des Editors (für `--tab`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Tab {
    Source,
    Dependencies,
    Package,
    Features,
    Advanced,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Source,
        Tab::Dependencies,
        Tab::Package,
        Tab::Features,
        Tab::Advanced,
    ];

    pub fn index(self) -> i32 {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0) as i32
    }
}

/// Darstellung (für `--theme` und das Menü „Darstellung“).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Theme {
    /// Wie das System.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    /// Name auf der Kommandozeile und in den Einstellungen.
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Theme::ALL.into_iter().find(|t| t.as_str() == text)
    }
}

/// CT-Editor – visueller Editor für Cargo.toml (Freya TDE).
#[derive(Debug, Clone, Parser)]
#[command(name = "ct-editor", version, about)]
pub struct Cli {
    /// Projektordner (enthält die Cargo.toml oder bekommt eine neue) oder die Cargo.toml selbst.
    #[arg(value_name = "PROJEKTORDNER", conflicts_with = "path")]
    pub project: Option<PathBuf>,

    /// Projektordner (alternativ zur Positionsangabe).
    #[arg(short, long, value_name = "ORDNER")]
    pub path: Option<PathBuf>,

    /// Sprache der Oberfläche, z. B. `de`, `en`, `de_DE.UTF-8`.
    /// Standard: Umgebungsvariable LANG, sonst Englisch.
    #[arg(short, long, value_name = "SPRACHE")]
    pub lang: Option<String>,

    /// Darstellung: system (Standard), light oder dark.
    /// Standard: gespeicherte Wahl (eigenständig), sonst wie das System.
    #[arg(long, value_enum, value_name = "DARSTELLUNG")]
    pub theme: Option<Theme>,

    /// Ordner mit zusätzlichen Übersetzungen (`<sprache>.toml`).
    #[arg(long, value_name = "ORDNER")]
    pub locales: Option<PathBuf>,

    /// Registerkarte beim Start.
    #[arg(long, value_enum, default_value = "dependencies")]
    pub tab: Tab,

    /// Ohne Netzwerk: keine Suche auf crates.io.
    #[arg(long)]
    pub offline: bool,

    /// Von Freya TDE gestartet: ohne Menü „Datei“ (Freya TDE bestimmt Datei
    /// und Sprache), die Sprachwahl wird nicht gespeichert.
    #[arg(long)]
    pub embedded: bool,

    /// Fenster als Bild speichern und beenden (Dokumentation, KI-Sichtprüfung).
    #[arg(long, value_name = "PNG")]
    pub screenshot: Option<PathBuf>,

    /// Fenstergröße als BREITExHÖHE, z. B. 1100x720.
    #[arg(long, value_name = "BxH", default_value = "1100x720", value_parser = parse_size)]
    pub size: (i32, i32),
}

impl Cli {
    /// Der Projektordner: Angabe oder aktuelles Verzeichnis.
    ///
    /// Eine übergebene `Cargo.toml` (z. B. „Öffnen mit“ im Dateimanager) steht
    /// für ihren Ordner.
    pub fn project_dir(&self) -> PathBuf {
        let given = self
            .project
            .clone()
            .or_else(|| self.path.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        if given.file_name().is_some_and(|n| n == "Cargo.toml") && !given.is_dir() {
            given
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), PathBuf::from)
        } else {
            given
        }
    }

    /// Die gewünschte Sprache: Angabe, sonst `LANG`, sonst Englisch.
    pub fn language(&self) -> String {
        self.language_with(None)
    }

    /// Wie [`Cli::language`], mit der gespeicherten Wahl (`saved`) zwischen
    /// `--lang` und `LANG`: der eigenständige Editor merkt sich seine Sprache.
    pub fn language_with(&self, saved: Option<&str>) -> String {
        self.lang
            .clone()
            .or_else(|| saved.map(str::to_owned))
            .or_else(|| std::env::var("LANG").ok())
            .filter(|l| !l.is_empty() && l != "C" && l != "POSIX")
            .unwrap_or_else(|| crate::i18n::FALLBACK.to_owned())
    }
}

fn parse_size(text: &str) -> Result<(i32, i32), String> {
    let (w, h) = text
        .split_once(['x', 'X'])
        .ok_or_else(|| "Format BREITExHÖHE, z. B. 1100x720".to_owned())?;
    let w: i32 = w.trim().parse().map_err(|e| format!("Breite: {e}"))?;
    let h: i32 = h.trim().parse().map_err(|e| format!("Höhe: {e}"))?;
    if w < 400 || h < 300 {
        return Err("mindestens 400x300".to_owned());
    }
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_file_stands_for_its_folder() {
        let cli = Cli::parse_from(["ct-editor", "/tmp/p/Cargo.toml"]);
        assert_eq!(cli.project_dir(), PathBuf::from("/tmp/p"));
        let cli = Cli::parse_from(["ct-editor", "Cargo.toml"]);
        assert_eq!(cli.project_dir(), PathBuf::from("."));
    }

    #[test]
    fn freya_style_call() {
        let cli = Cli::parse_from(["ct-editor", "--lang", "de", "--path", "/tmp/p"]);
        assert_eq!(cli.project_dir(), PathBuf::from("/tmp/p"));
        assert_eq!(cli.language(), "de");
        assert_eq!(cli.tab, Tab::Dependencies);
        assert_eq!(cli.size, (1100, 720));
        assert_eq!(cli.theme, None);
        let cli = Cli::parse_from(["ct-editor", "--embedded", "--theme", "dark", "/tmp/p"]);
        assert_eq!(cli.theme, Some(Theme::Dark));
        assert_eq!(Theme::parse("light"), Some(Theme::Light));
    }

    #[test]
    fn positional_and_options() {
        let cli = Cli::parse_from(["ct-editor", "proj", "--tab", "source", "--size", "800x600"]);
        assert_eq!(cli.project_dir(), PathBuf::from("proj"));
        assert_eq!(cli.tab.index(), 0);
        assert_eq!(cli.size, (800, 600));
        assert!(Cli::try_parse_from(["ct-editor", "a", "--path", "b"]).is_err());
        assert!(Cli::try_parse_from(["ct-editor", "--size", "10x10"]).is_err());
    }
}
