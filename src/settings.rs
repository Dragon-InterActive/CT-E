//! Persönliche Einstellungen des eigenständigen Editors (vorerst die Sprache),
//! ohne Oberfläche. Gespeichert als TOML in `<config>/ct-editor/settings.toml`
//! (Linux `$XDG_CONFIG_HOME` bzw. `~/.config`, macOS
//! `~/Library/Application Support`, Windows `%APPDATA%`); `CT_EDITOR_CONFIG`
//! nennt eine andere Datei (Tests, Automatisierung).
//!
//! ```
//! use ct_editor::settings::Settings;
//!
//! let file = std::env::temp_dir().join(format!("ct-settings-doc-{}.toml", std::process::id()));
//! Settings { language: Some("de".into()) }.save_to(&file).unwrap();
//! assert_eq!(Settings::load_from(&file).language.as_deref(), Some("de"));
//! # let _ = std::fs::remove_file(file);
//! ```

use std::path::{Path, PathBuf};
use std::str::FromStr;

use toml_edit::{DocumentMut, value};

/// Die Einstellungen; fehlende Werte sind `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    /// Sprache der Oberfläche (`de`, `en` …), gewählt im Menü „Sprache“.
    pub language: Option<String>,
}

impl Settings {
    /// Die Datei der Einstellungen (`None`, wenn kein Konfigurationsordner bekannt ist).
    pub fn file() -> Option<PathBuf> {
        if let Some(file) = std::env::var_os("CT_EDITOR_CONFIG").filter(|f| !f.is_empty()) {
            return Some(PathBuf::from(file));
        }
        config_dir().map(|dir| dir.join("ct-editor").join("settings.toml"))
    }

    /// Liest die Einstellungen; eine fehlende oder kaputte Datei ergibt die Vorgaben.
    pub fn load() -> Self {
        Self::file()
            .map(|f| Self::load_from(&f))
            .unwrap_or_default()
    }

    /// Speichert die Einstellungen (legt den Ordner bei Bedarf an).
    pub fn save(&self) -> Result<(), String> {
        let file = Self::file().ok_or("kein Konfigurationsordner")?;
        self.save_to(&file)
    }

    pub fn load_from(file: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(file) else {
            return Self::default();
        };
        let Ok(doc) = DocumentMut::from_str(&text) else {
            return Self::default();
        };
        Self {
            language: doc
                .get("language")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
        }
    }

    /// Schreibt die Datei; andere Einträge darin bleiben erhalten.
    pub fn save_to(&self, file: &Path) -> Result<(), String> {
        let mut doc = std::fs::read_to_string(file)
            .ok()
            .and_then(|t| DocumentMut::from_str(&t).ok())
            .unwrap_or_default();
        match &self.language {
            Some(language) => doc["language"] = value(language.as_str()),
            None => {
                doc.remove("language");
            }
        }
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(file, doc.to_string()).map_err(|e| format!("{}: {e}", file.display()))
    }
}

/// Der Konfigurationsordner des Systems.
fn config_dir() -> Option<PathBuf> {
    let env = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_other_entries() {
        let file = std::env::temp_dir().join(format!("ct-settings-{}.toml", std::process::id()));
        std::fs::write(&file, "# eigene Notiz\nother = 1\n").unwrap();
        Settings {
            language: Some("en".into()),
        }
        .save_to(&file)
        .unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            text.contains("other = 1") && text.contains("# eigene Notiz"),
            "{text}"
        );
        assert_eq!(Settings::load_from(&file).language.as_deref(), Some("en"));
        Settings::default().save_to(&file).unwrap();
        assert_eq!(Settings::load_from(&file), Settings::default());
        std::fs::write(&file, "kaputt = [").unwrap();
        assert_eq!(Settings::load_from(&file), Settings::default());
        let _ = std::fs::remove_file(&file);
    }
}
