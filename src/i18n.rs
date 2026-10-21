//! Mehrsprachigkeit (i18n).
//!
//! Übersetzungen liegen als TOML-Dateien unter `locales/<sprache>.toml` und
//! werden in das Programm eingebettet. Freya TD kann zusätzlich einen Ordner
//! mit weiteren `*.toml`-Dateien übergeben (`--locales`), um Sprachen ohne
//! Neukompilieren nachzurüsten oder Texte zu überschreiben.
//!
//! Schlüssel sind verschachtelte Tabellen in Punktschreibweise
//! (`[deps] search = "…"` → `deps.search`). Platzhalter stehen in
//! geschweiften Klammern: `"Zeile {line}"`.
//!
//! ```
//! use ct_editor::{i18n, tr};
//!
//! i18n::set_language("de");
//! assert_eq!(tr!("app.save"), "Speichern");
//! assert_eq!(tr!("status.parse_error", line = 3, message = "x"), "Syntaxfehler in Zeile 3: x");
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::{OnceLock, RwLock};

use toml_edit::{DocumentMut, Item};

/// Sprache, auf die zurückgegriffen wird, wenn ein Text fehlt.
pub const FALLBACK: &str = "en";

const EMBEDDED: &[(&str, &str)] = &[
    ("de", include_str!("../locales/de.toml")),
    ("en", include_str!("../locales/en.toml")),
];

struct Catalog {
    language: String,
    texts: HashMap<String, HashMap<String, String>>,
}

fn catalog() -> &'static RwLock<Catalog> {
    static CATALOG: OnceLock<RwLock<Catalog>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut texts = HashMap::new();
        for (lang, source) in EMBEDDED {
            let parsed = parse(source).unwrap_or_else(|e| panic!("locales/{lang}.toml: {e}"));
            texts.insert((*lang).to_owned(), parsed);
        }
        RwLock::new(Catalog {
            language: FALLBACK.to_owned(),
            texts,
        })
    })
}

fn parse(source: &str) -> Result<HashMap<String, String>, String> {
    let doc = DocumentMut::from_str(source).map_err(|e| e.to_string())?;
    let mut out = HashMap::new();
    flatten("", doc.as_item(), &mut out);
    Ok(out)
}

fn flatten(prefix: &str, item: &Item, out: &mut HashMap<String, String>) {
    if let Some(table) = item.as_table_like() {
        for (key, child) in table.iter() {
            let path = if prefix.is_empty() {
                key.to_owned()
            } else {
                format!("{prefix}.{key}")
            };
            flatten(&path, child, out);
        }
    } else if let Some(text) = item.as_str() {
        out.insert(prefix.to_owned(), text.to_owned());
    }
}

/// Normalisiert Sprachangaben wie `de_DE.UTF-8` oder `de-AT` zu Kandidaten
/// (`de-de`, `de`) in absteigender Genauigkeit.
fn candidates(language: &str) -> Vec<String> {
    let base = language
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-")
        .to_lowercase();
    let mut out = vec![base.clone()];
    if let Some((primary, _)) = base.split_once('-') {
        out.push(primary.to_owned());
    }
    out
}

/// Stellt die Sprache ein und liefert die tatsächlich verwendete.
///
/// Unbekannte Sprachen fallen auf die Hauptsprache (`de-AT` → `de`) und
/// zuletzt auf [`FALLBACK`] zurück.
pub fn set_language(language: &str) -> String {
    let mut catalog = catalog().write().expect("i18n-Katalog vergiftet");
    let chosen = candidates(language)
        .into_iter()
        .find(|c| catalog.texts.contains_key(c))
        .unwrap_or_else(|| FALLBACK.to_owned());
    catalog.language = chosen.clone();
    chosen
}

/// Die aktuell eingestellte Sprache.
pub fn language() -> String {
    catalog()
        .read()
        .expect("i18n-Katalog vergiftet")
        .language
        .clone()
}

/// Alle verfügbaren Sprachen (sortiert).
pub fn available() -> Vec<String> {
    let catalog = catalog().read().expect("i18n-Katalog vergiftet");
    let mut langs: Vec<String> = catalog.texts.keys().cloned().collect();
    langs.sort();
    langs
}

/// Lädt alle `*.toml`-Dateien aus `dir` als zusätzliche Sprachen. Vorhandene
/// Sprachen werden ergänzt bzw. Texte überschrieben. Liefert die geladenen
/// Sprachen; fehlerhafte Dateien werden mit Grund gemeldet.
pub fn load_dir(dir: &Path) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut loaded = Vec::new();
    let mut catalog = catalog().write().expect("i18n-Katalog vergiftet");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Some(lang) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let source =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let texts = parse(&source).map_err(|e| format!("{}: {e}", path.display()))?;
        let lang = candidates(lang).remove(0);
        catalog.texts.entry(lang.clone()).or_default().extend(texts);
        loaded.push(lang);
    }
    loaded.sort();
    Ok(loaded)
}

/// Übersetzt `key` in die aktuelle Sprache. Fehlt der Text, wird die
/// Rückfallsprache und zuletzt der Schlüssel selbst geliefert – so fallen
/// fehlende Übersetzungen sofort auf, ohne dass etwas abstürzt.
pub fn translate(key: &str) -> String {
    let catalog = catalog().read().expect("i18n-Katalog vergiftet");
    [catalog.language.as_str(), FALLBACK]
        .iter()
        .find_map(|lang| catalog.texts.get(*lang)?.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}

/// Wie [`translate`], ersetzt aber `{name}`-Platzhalter.
pub fn translate_with(key: &str, args: &[(&str, String)]) -> String {
    let mut text = translate(key);
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

/// Alle Schlüssel einer Sprache (für Vollständigkeitstests).
pub fn keys(language: &str) -> Vec<String> {
    let catalog = catalog().read().expect("i18n-Katalog vergiftet");
    let mut keys: Vec<String> = catalog
        .texts
        .get(language)
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    keys
}

/// Übersetzt einen Schlüssel: `tr!("app.save")` oder mit Platzhaltern
/// `tr!("status.saved", path = p.display())`.
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::i18n::translate($key)
    };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::translate_with($key, &[$((stringify!($name), $value.to_string())),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_complete() {
        let reference = keys(FALLBACK);
        assert!(!reference.is_empty());
        for lang in available() {
            assert_eq!(keys(&lang), reference, "Sprache {lang} unvollständig");
        }
    }

    #[test]
    fn every_used_key_exists() {
        // Alle tr!("…")-Aufrufe im Quelltext müssen übersetzt sein.
        let sources = [
            include_str!("main.rs"),
            include_str!("ui/mod.rs"),
            include_str!("ui/source_tab.rs"),
            include_str!("ui/deps_tab.rs"),
            include_str!("ui/package_tab.rs"),
            include_str!("ui/features_tab.rs"),
            include_str!("ui/advanced_tab.rs"),
            include_str!("crates_io.rs"),
            include_str!("cargo_cli.rs"),
        ];
        let known = keys(FALLBACK);
        for source in sources {
            for part in source.split("tr!(\"").skip(1) {
                let key = part.split('"').next().unwrap();
                assert!(known.iter().any(|k| k == key), "Schlüssel fehlt: {key}");
            }
        }
    }

    #[test]
    fn package_field_tooltips_exist() {
        let known = keys(FALLBACK);
        for field in crate::manifest::PACKAGE_FIELDS {
            let key = format!("package.field.{}", field.key);
            assert!(known.contains(&key), "Schlüssel fehlt: {key}");
        }
        for key in ["deps.added", "deps.updated", "source.error"] {
            assert!(known.iter().any(|k| k == key), "Schlüssel fehlt: {key}");
        }
    }

    #[test]
    fn candidates_and_fallback() {
        assert_eq!(candidates("de_DE.UTF-8"), ["de-de", "de"]);
        assert_eq!(translate_with("x.unknown", &[]), "x.unknown");
    }
}
