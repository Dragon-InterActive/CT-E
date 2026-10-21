//! Das Manifest-Modell: eine `Cargo.toml` als formatierungserhaltendes Dokument.
//!
//! Grundlage ist `toml_edit` – dieselbe Bibliothek, mit der Cargo selbst
//! (`cargo add`, `cargo remove`) Manifeste bearbeitet. Kommentare, Leerzeilen
//! und die Reihenfolge der Einträge bleiben deshalb erhalten; geändert wird
//! nur, was der Anwender tatsächlich ändert.
//!
//! Dieses Modul kennt keine Oberfläche und ist vollständig ohne Qt testbar.

use std::fmt;
use std::str::FromStr;

use toml_edit::{Array, DocumentMut, InlineTable, Item, Key, Table, TableLike, Value};

/// Ein Syntaxfehler mit Position (Zeile und Spalte ab 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl ParseError {
    fn from_toml(text: &str, error: &toml_edit::TomlError) -> Self {
        let offset = error.span().map(|s| s.start).unwrap_or(0).min(text.len());
        let before = &text[..offset];
        let line = before.matches('\n').count();
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count());
        Self {
            line,
            column,
            message: error.message().trim().to_owned(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line + 1, self.column + 1, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Die Art einer Abhängigkeitstabelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepKind {
    Normal,
    Dev,
    Build,
}

impl DepKind {
    pub const ALL: [DepKind; 3] = [DepKind::Normal, DepKind::Dev, DepKind::Build];

    /// Der Tabellenname im Manifest.
    pub fn table_name(self) -> &'static str {
        match self {
            DepKind::Normal => "dependencies",
            DepKind::Dev => "dev-dependencies",
            DepKind::Build => "build-dependencies",
        }
    }
}

/// Wo eine Abhängigkeit steht: Art plus optional eine Zielplattform
/// (`[target.'cfg(unix)'.dependencies]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DepTable {
    pub kind: DepKind,
    pub target: Option<String>,
}

impl DepTable {
    pub fn new(kind: DepKind) -> Self {
        Self { kind, target: None }
    }

    pub fn for_target(kind: DepKind, target: impl Into<String>) -> Self {
        let target = target.into();
        Self {
            kind,
            target: (!target.trim().is_empty()).then(|| target.trim().to_owned()),
        }
    }
}

impl fmt::Display for DepTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target {
            Some(target) => write!(f, "target.'{target}'.{}", self.kind.table_name()),
            None => f.write_str(self.kind.table_name()),
        }
    }
}

/// Eine Abhängigkeit, wie sie im Manifest steht.
///
/// Felder, die der Editor nicht kennt (z. B. `registry`), bleiben beim
/// Zurückschreiben unangetastet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Der Schlüssel in der Tabelle (bei Umbenennung der neue Name).
    pub name: String,
    /// Der echte Crate-Name bei Umbenennung (`package = "…"`).
    pub package: Option<String>,
    pub version: Option<String>,
    pub path: Option<String>,
    pub git: Option<String>,
    /// `dep = { workspace = true }`: Version kommt aus `[workspace.dependencies]`.
    pub workspace: bool,
    pub features: Vec<String>,
    pub default_features: bool,
    pub optional: bool,
}

impl Dependency {
    /// Eine schlichte Abhängigkeit `name = "version"`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            package: None,
            version: Some(version.into()),
            path: None,
            git: None,
            workspace: false,
            features: Vec::new(),
            default_features: true,
            optional: false,
        }
    }

    /// Der Name auf crates.io (berücksichtigt `package = "…"`).
    pub fn crate_name(&self) -> &str {
        self.package.as_deref().unwrap_or(&self.name)
    }

    /// Kurzform für Listen: `serde 1.0 [derive]`.
    pub fn summary(&self) -> String {
        let mut text = self.name.clone();
        if self.workspace {
            text.push_str(" (workspace)");
        } else if let Some(version) = &self.version {
            text.push(' ');
            text.push_str(version);
        } else if let Some(path) = &self.path {
            text.push_str(&format!(" (path: {path})"));
        } else if let Some(git) = &self.git {
            text.push_str(&format!(" (git: {git})"));
        }
        if !self.default_features {
            text.push_str(" !default");
        }
        if !self.features.is_empty() {
            text.push_str(&format!(" [{}]", self.features.join(", ")));
        }
        if self.optional {
            text.push_str(" optional");
        }
        text
    }

    fn from_item(name: &str, item: &Item) -> Option<Self> {
        if let Some(version) = item.as_str() {
            return Some(Self::new(name, version));
        }
        let table = item.as_table_like()?;
        let text = |key: &str| table.get(key).and_then(Item::as_str).map(str::to_owned);
        let flag = |key: &str| table.get(key).and_then(Item::as_bool);
        let features = table
            .get("features")
            .and_then(Item::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            name: name.to_owned(),
            package: text("package"),
            version: text("version"),
            path: text("path"),
            git: text("git"),
            workspace: flag("workspace").unwrap_or(false),
            features,
            // Cargo akzeptiert noch die alte Schreibweise mit Unterstrich.
            default_features: flag("default-features")
                .or_else(|| flag("default_features"))
                .unwrap_or(true),
            optional: flag("optional").unwrap_or(false),
        })
    }
}

/// Der Wert eines `[package]`-Feldes, aufbereitet für ein Formular.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    Missing,
    Text(String),
    List(Vec<String>),
    Bool(bool),
    /// `key.workspace = true` – vom Workspace geerbt.
    Inherited,
    /// Etwas anderes (z. B. `publish = ["meine-registry"]`) – nur im Quelltext bearbeitbar.
    Complex(String),
}

/// Wie ein `[package]`-Feld im Formular bearbeitet wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    /// Kommagetrennte Liste (z. B. `keywords`).
    List,
    /// Auswahl aus festen Werten, Freitext erlaubt.
    Choice(&'static [&'static str]),
    Bool {
        default: bool,
    },
}

/// Ein Feld des `[package]`-Abschnitts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageField {
    pub key: &'static str,
    pub kind: FieldKind,
}

/// Alle Felder des `[package]`-Abschnitts, die das Formular anbietet – in
/// der Reihenfolge der Cargo-Dokumentation. Alles Weitere (`metadata`,
/// `include`, …) bleibt im Quelltext bzw. im Tab "Erweitert" bearbeitbar.
pub const PACKAGE_FIELDS: &[PackageField] = &[
    PackageField {
        key: "name",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "version",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "edition",
        kind: FieldKind::Choice(&["2015", "2018", "2021", "2024"]),
    },
    PackageField {
        key: "rust-version",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "description",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "authors",
        kind: FieldKind::List,
    },
    PackageField {
        key: "license",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "license-file",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "readme",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "homepage",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "repository",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "documentation",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "keywords",
        kind: FieldKind::List,
    },
    PackageField {
        key: "categories",
        kind: FieldKind::List,
    },
    PackageField {
        key: "build",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "links",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "default-run",
        kind: FieldKind::Text,
    },
    PackageField {
        key: "resolver",
        kind: FieldKind::Choice(&["1", "2", "3"]),
    },
    PackageField {
        key: "publish",
        kind: FieldKind::Bool { default: true },
    },
    PackageField {
        key: "autobins",
        kind: FieldKind::Bool { default: true },
    },
    PackageField {
        key: "autoexamples",
        kind: FieldKind::Bool { default: true },
    },
    PackageField {
        key: "autotests",
        kind: FieldKind::Bool { default: true },
    },
    PackageField {
        key: "autobenches",
        kind: FieldKind::Bool { default: true },
    },
];

/// Vorlagen für neue Abschnitte im Tab "Erweitert": (Schlüssel, TOML-Text).
pub const SECTION_TEMPLATES: &[(&str, &str)] = &[
    ("lib", "[lib]\nname = \"\"\npath = \"src/lib.rs\"\n"),
    ("bin", "[[bin]]\nname = \"\"\npath = \"src/main.rs\"\n"),
    (
        "example",
        "[[example]]\nname = \"\"\npath = \"examples/.rs\"\n",
    ),
    ("test", "[[test]]\nname = \"\"\npath = \"tests/.rs\"\n"),
    ("bench", "[[bench]]\nname = \"\"\nharness = false\n"),
    (
        "profile",
        "[profile.release]\nopt-level = 3\nlto = true\ncodegen-units = 1\n",
    ),
    ("workspace", "[workspace]\nresolver = \"3\"\nmembers = []\n"),
    (
        "lints",
        "[lints.rust]\nunsafe_code = \"forbid\"\n\n[lints.clippy]\nall = \"warn\"\n",
    ),
    ("patch", "[patch.crates-io]\n"),
    ("replace", "[replace]\n"),
    (
        "badges",
        "[badges]\nmaintenance = { status = \"actively-developed\" }\n",
    ),
    ("target", "[target.'cfg(unix)'.dependencies]\n"),
];

/// Eine `Cargo.toml` im Speicher.
#[derive(Debug, Clone)]
pub struct Manifest {
    doc: DocumentMut,
}

impl Manifest {
    /// Liest ein Manifest; bei Syntaxfehlern mit Zeile und Spalte.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        DocumentMut::from_str(text)
            .map(|doc| Self { doc })
            .map_err(|e| ParseError::from_toml(text, &e))
    }

    /// Ein neues Paket-Manifest, wie `cargo new` es anlegt.
    pub fn new_package(name: &str) -> Self {
        let text = format!(
            "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n",
            sanitize_package_name(name)
        );
        Self::parse(&text).expect("Vorlage ist gültiges TOML")
    }

    /// Das Manifest als Text – unveränderte Teile Byte für Byte wie gelesen.
    pub fn to_text(&self) -> String {
        self.doc.to_string()
    }

    /// Direkter Zugriff für Sonderfälle, die das Modell nicht abdeckt.
    pub fn document(&self) -> &DocumentMut {
        &self.doc
    }

    pub fn document_mut(&mut self) -> &mut DocumentMut {
        &mut self.doc
    }

    // --- [package] ----------------------------------------------------------

    pub fn has_package(&self) -> bool {
        self.doc.get("package").is_some_and(Item::is_table_like)
    }

    /// Legt einen leeren `[package]`-Abschnitt am Anfang an (falls keiner existiert).
    pub fn ensure_package(&mut self) {
        if !self.has_package() {
            let mut table = Table::new();
            // Vor allen anderen Abschnitten ausgeben.
            table.set_position(Some(-1));
            self.doc.insert("package", Item::Table(table));
        }
    }

    pub fn package_field(&self, key: &str) -> FieldValue {
        let Some(item) = self.doc.get("package").and_then(|p| p.get(key)) else {
            return FieldValue::Missing;
        };
        if let Some(table) = item.as_table_like()
            && table.get("workspace").and_then(Item::as_bool) == Some(true)
        {
            return FieldValue::Inherited;
        }
        match item.as_value() {
            Some(Value::String(s)) => FieldValue::Text(s.value().clone()),
            Some(Value::Boolean(b)) => FieldValue::Bool(*b.value()),
            Some(Value::Array(a)) if a.iter().all(Value::is_str) => FieldValue::List(
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect(),
            ),
            _ => FieldValue::Complex(item.to_string().trim().to_owned()),
        }
    }

    /// Setzt ein Textfeld; leerer Text entfernt das Feld.
    pub fn set_package_text(&mut self, key: &str, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            self.remove_package_field(key);
        } else {
            self.set_package_value(key, Value::from(text));
        }
    }

    /// Setzt ein Listenfeld; eine leere Liste entfernt das Feld.
    pub fn set_package_list(&mut self, key: &str, items: &[String]) {
        let items: Vec<&str> = items
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if items.is_empty() {
            self.remove_package_field(key);
        } else {
            self.set_package_value(key, Value::Array(items.into_iter().collect::<Array>()));
        }
    }

    /// Setzt ein Bool-Feld; der Standardwert wird nicht ausgeschrieben.
    pub fn set_package_bool(&mut self, key: &str, value: bool, default: bool) {
        if value == default {
            self.remove_package_field(key);
        } else {
            self.set_package_value(key, Value::from(value));
        }
    }

    pub fn remove_package_field(&mut self, key: &str) {
        if let Some(package) = self
            .doc
            .get_mut("package")
            .and_then(Item::as_table_like_mut)
        {
            package.remove(key);
        }
    }

    fn set_package_value(&mut self, key: &str, value: Value) {
        self.ensure_package();
        let package = self.doc["package"]
            .as_table_like_mut()
            .expect("ensure_package legt eine Tabelle an");
        set_value_keep_decor(package, key, value);
    }

    // --- Abhängigkeiten -----------------------------------------------------

    /// Alle vorhandenen Abhängigkeitstabellen, plattformspezifische eingeschlossen.
    pub fn dependency_tables(&self) -> Vec<DepTable> {
        let mut tables: Vec<DepTable> = DepKind::ALL
            .into_iter()
            .filter(|k| {
                self.doc
                    .get(k.table_name())
                    .is_some_and(Item::is_table_like)
            })
            .map(DepTable::new)
            .collect();
        if let Some(targets) = self.doc.get("target").and_then(Item::as_table_like) {
            for (cfg, item) in targets.iter() {
                for kind in DepKind::ALL {
                    if item.get(kind.table_name()).is_some_and(Item::is_table_like) {
                        tables.push(DepTable::for_target(kind, cfg));
                    }
                }
            }
        }
        tables
    }

    /// Alle Plattformen aus `[target.*]` (z. B. `cfg(unix)`).
    pub fn targets(&self) -> Vec<String> {
        self.doc
            .get("target")
            .and_then(Item::as_table_like)
            .map(|t| t.iter().map(|(k, _)| k.to_owned()).collect())
            .unwrap_or_default()
    }

    fn dep_table(&self, table: &DepTable) -> Option<&dyn TableLike> {
        let item = match &table.target {
            Some(target) => self
                .doc
                .get("target")?
                .get(target)?
                .get(table.kind.table_name())?,
            None => self.doc.get(table.kind.table_name())?,
        };
        item.as_table_like()
    }

    fn dep_table_mut(&mut self, table: &DepTable, create: bool) -> Option<&mut dyn TableLike> {
        let name = table.kind.table_name();
        let parent: &mut dyn TableLike = match &table.target {
            Some(target) => {
                if create {
                    let targets = self.doc.entry("target").or_insert_with(implicit_table);
                    let targets = targets.as_table_like_mut()?;
                    if targets.get(target).is_none() {
                        // Wie in der Cargo-Doku: 'cfg(unix)' in einfachen Anführungszeichen.
                        let key = format!("'{target}'")
                            .parse::<Key>()
                            .unwrap_or_else(|_| Key::new(target.as_str()));
                        targets.entry_format(&key).or_insert_with(implicit_table);
                    }
                }
                self.doc
                    .get_mut("target")?
                    .get_mut(target)?
                    .as_table_like_mut()?
            }
            None => self.doc.as_table_mut(),
        };
        if create && parent.get(name).is_none() {
            parent.insert(name, Item::Table(Table::new()));
        }
        parent.get_mut(name)?.as_table_like_mut()
    }

    /// Alle Abhängigkeiten einer Tabelle in Dateireihenfolge.
    pub fn dependencies(&self, table: &DepTable) -> Vec<Dependency> {
        self.dep_table(table)
            .map(|t| {
                t.iter()
                    .filter_map(|(name, item)| Dependency::from_item(name, item))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn dependency(&self, table: &DepTable, name: &str) -> Option<Dependency> {
        let item = self.dep_table(table)?.get(name)?;
        Dependency::from_item(name, item)
    }

    /// Fügt eine Abhängigkeit hinzu oder aktualisiert sie.
    ///
    /// Wie `cargo add`: die kürzeste gültige Form (`name = "1.0"`), sonst eine
    /// Inline-Tabelle; eine bestehende `[dependencies.name]`-Tabelle bleibt
    /// eine Tabelle. Unbekannte Schlüssel einer bestehenden Abhängigkeit bleiben
    /// erhalten. War die Tabelle alphabetisch sortiert, bleibt sie es.
    pub fn set_dependency(&mut self, table: &DepTable, dep: &Dependency) {
        let deps = self
            .dep_table_mut(table, true)
            .expect("Tabelle wurde angelegt");
        let was_sorted = is_sorted(deps);
        let is_new = deps.get(&dep.name).is_none();

        match deps.get_mut(&dep.name) {
            Some(item) if item.is_table_like() => {
                write_dependency_keys(item.as_table_like_mut().expect("geprüft"), dep);
                if let Some(inline) = item.as_inline_table_mut() {
                    inline.fmt();
                }
                // Nur noch "version" übrig? Dann zur Kurzform zusammenfalten.
                if let Some(t) = item.as_inline_table()
                    && t.len() == 1
                    && let Some(version) = t.get("version").and_then(Value::as_str)
                {
                    let version = version.to_owned();
                    replace_item_keep_decor(item, Value::from(version));
                }
            }
            _ => {
                let simple = dep.package.is_none()
                    && dep.path.is_none()
                    && dep.git.is_none()
                    && !dep.workspace
                    && dep.features.is_empty()
                    && dep.default_features
                    && !dep.optional;
                let value = match (&dep.version, simple) {
                    (Some(version), true) => Value::from(version.as_str()),
                    _ => {
                        let mut inline = InlineTable::new();
                        write_dependency_keys(&mut inline, dep);
                        Value::InlineTable(inline)
                    }
                };
                set_value_keep_decor(deps, &dep.name, value);
            }
        }
        if is_new && was_sorted {
            deps.sort_values();
        }
    }

    /// Entfernt eine Abhängigkeit; `true`, wenn es sie gab.
    pub fn remove_dependency(&mut self, table: &DepTable, name: &str) -> bool {
        self.dep_table_mut(table, false)
            .and_then(|t| t.remove(name))
            .is_some()
    }

    // --- [features] ---------------------------------------------------------

    /// Alle eigenen Features: (Name, aktivierte Einträge).
    pub fn features(&self) -> Vec<(String, Vec<String>)> {
        self.doc
            .get("features")
            .and_then(Item::as_table_like)
            .map(|t| {
                t.iter()
                    .map(|(name, item)| {
                        let entries = item
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(str::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default();
                        (name.to_owned(), entries)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Legt ein Feature an oder ersetzt seine Einträge.
    pub fn set_feature(&mut self, name: &str, entries: &[String]) {
        let features = self
            .doc
            .entry("features")
            .or_insert_with(|| Item::Table(Table::new()));
        let Some(features) = features.as_table_like_mut() else {
            return;
        };
        let array: Array = entries
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        set_value_keep_decor(features, name, Value::Array(array));
    }

    pub fn remove_feature(&mut self, name: &str) -> bool {
        self.doc
            .get_mut("features")
            .and_then(Item::as_table_like_mut)
            .and_then(|t| t.remove(name))
            .is_some()
    }

    // --- Abschnitte (Tab "Erweitert") ---------------------------------------

    /// Alle Schlüssel der obersten Ebene in Dateireihenfolge.
    pub fn sections(&self) -> Vec<String> {
        let mut keys: Vec<(isize, String)> = self
            .doc
            .iter()
            .enumerate()
            .map(|(i, (k, item))| (section_position(item).unwrap_or(i as isize), k.to_owned()))
            .collect();
        keys.sort_by_key(|(pos, _)| *pos);
        keys.into_iter().map(|(_, k)| k).collect()
    }

    /// Ein Abschnitt als eigenständiger TOML-Text.
    pub fn section_text(&self, key: &str) -> String {
        let Some(item) = self.doc.get(key) else {
            return String::new();
        };
        let mut doc = DocumentMut::new();
        doc.insert(key, item.clone());
        doc.to_string().trim_start().to_owned()
    }

    /// Ersetzt den Abschnitt `key` durch den Inhalt von `text`.
    ///
    /// `text` darf mehrere Schlüssel der obersten Ebene enthalten (z. B. nach
    /// dem Einfügen einer Vorlage); fehlt `key` darin, wird der Abschnitt
    /// entfernt. Ein leerer Text entfernt ihn ebenfalls.
    pub fn set_section_text(&mut self, key: &str, text: &str) -> Result<(), ParseError> {
        let snippet = DocumentMut::from_str(text).map_err(|e| ParseError::from_toml(text, &e))?;
        let position = self.doc.get(key).and_then(section_position);
        let decor = self
            .doc
            .get(key)
            .and_then(Item::as_table)
            .map(|t| t.decor().clone());
        if !snippet.contains_key(key) {
            self.doc.remove(key);
        }
        for (name, item) in snippet.iter() {
            let mut item = item.clone();
            // An der alten Stelle bleiben statt ans Dateiende zu wandern.
            if name == key
                && let Some(table) = item.as_table_mut()
            {
                table.set_position(position);
                // Leerzeilen/Kommentare vor der Überschrift übernehmen.
                if let Some(decor) = &decor {
                    *table.decor_mut() = decor.clone();
                }
            }
            self.doc.insert(name, item);
        }
        Ok(())
    }

    pub fn remove_section(&mut self, key: &str) -> bool {
        self.doc.remove(key).is_some()
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new_package("neues-projekt")
    }
}

impl fmt::Display for Manifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

/// Macht aus einem Ordnernamen einen gültigen Paketnamen (wie `cargo new`).
pub fn sanitize_package_name(name: &str) -> String {
    let mut out: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "pkg-");
    }
    out
}

/// Zerlegt eine kommagetrennte Eingabe in Einträge.
pub fn split_list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn implicit_table() -> Item {
    let mut table = Table::new();
    table.set_implicit(true);
    Item::Table(table)
}

fn section_position(item: &Item) -> Option<isize> {
    match item {
        Item::Table(t) => t.position(),
        Item::ArrayOfTables(a) => a.iter().next().and_then(Table::position),
        _ => None,
    }
}

fn is_sorted(table: &dyn TableLike) -> bool {
    let keys: Vec<&str> = table.iter().map(|(k, _)| k).collect();
    keys.windows(2).all(|w| w[0] <= w[1])
}

/// Ersetzt einen Wert, behält aber Kommentare und Abstände des alten Werts.
fn set_value_keep_decor(table: &mut dyn TableLike, key: &str, value: Value) {
    match table.get_mut(key) {
        Some(item) => replace_item_keep_decor(item, value),
        None => {
            table.insert(key, Item::Value(value));
        }
    }
}

fn replace_item_keep_decor(item: &mut Item, mut value: Value) {
    if let Some(old) = item.as_value() {
        *value.decor_mut() = old.decor().clone();
    }
    *item = Item::Value(value);
}

/// Schreibt die vom Editor verwalteten Schlüssel einer Abhängigkeit;
/// Standardwerte werden entfernt, unbekannte Schlüssel bleiben.
fn write_dependency_keys(table: &mut dyn TableLike, dep: &Dependency) {
    fn put(table: &mut dyn TableLike, key: &str, value: Option<Value>) {
        match value {
            Some(v) => set_value_keep_decor(table, key, v),
            None => {
                table.remove(key);
            }
        }
    }
    let text = |s: &Option<String>| s.as_deref().map(Value::from);

    put(table, "package", text(&dep.package));
    put(table, "workspace", dep.workspace.then(|| Value::from(true)));
    put(
        table,
        "version",
        if dep.workspace {
            None
        } else {
            text(&dep.version)
        },
    );
    put(table, "path", text(&dep.path));
    put(table, "git", text(&dep.git));
    table.remove("default_features");
    put(
        table,
        "default-features",
        (!dep.default_features).then(|| Value::from(false)),
    );
    put(
        table,
        "features",
        (!dep.features.is_empty())
            .then(|| Value::Array(dep.features.iter().map(String::as_str).collect())),
    );
    put(table, "optional", dep.optional.then(|| Value::from(true)));
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# Mein Projekt
[package]
name = "demo"   # Paketname
version = "0.1.0"
edition = "2021"
keywords = ["a", "b"]
license.workspace = true

[dependencies]
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }

[dependencies.tokio]
version = "1"
features = ["full"]

[target.'cfg(unix)'.dependencies]
libc = "0.2"

[features]
default = ["std"]
std = []
"#;

    #[test]
    fn unchanged_round_trip() {
        assert_eq!(Manifest::parse(SAMPLE).unwrap().to_text(), SAMPLE);
    }

    #[test]
    fn parse_error_position() {
        let err = Manifest::parse("[package]\nname = \"x\nversion = 1").unwrap_err();
        assert_eq!(err.line, 1);
        assert!(!err.message.is_empty());
    }

    #[test]
    fn package_fields() {
        let mut m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(m.package_field("name"), FieldValue::Text("demo".into()));
        assert_eq!(
            m.package_field("keywords"),
            FieldValue::List(vec!["a".into(), "b".into()])
        );
        assert_eq!(m.package_field("license"), FieldValue::Inherited);
        assert_eq!(m.package_field("description"), FieldValue::Missing);

        m.set_package_text("name", "neu");
        // Der Kommentar hinter dem Wert bleibt erhalten.
        assert!(m.to_text().contains("name = \"neu\"   # Paketname"));
        m.set_package_text("description", "Beschreibung");
        m.set_package_text("version", "");
        m.set_package_list("keywords", &[]);
        m.set_package_bool("publish", false, true);
        let text = m.to_text();
        assert!(text.contains("description = \"Beschreibung\""));
        assert!(!text.contains("version = \"0.1.0\""));
        assert!(!text.contains("keywords"));
        assert!(text.contains("publish = false"));
        m.set_package_bool("publish", true, true);
        assert!(!m.to_text().contains("publish"));
        assert!(Manifest::parse(&m.to_text()).is_ok());
    }

    #[test]
    fn read_dependencies() {
        let m = Manifest::parse(SAMPLE).unwrap();
        let normal = DepTable::new(DepKind::Normal);
        let deps = m.dependencies(&normal);
        assert_eq!(
            deps.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
            ["anyhow", "serde", "tokio"]
        );
        assert_eq!(deps[1].features, ["derive"]);
        assert_eq!(deps[2].version.as_deref(), Some("1"));
        let tables = m.dependency_tables();
        assert!(tables.contains(&DepTable::for_target(DepKind::Normal, "cfg(unix)")));
        assert_eq!(m.targets(), ["cfg(unix)"]);
    }

    #[test]
    fn add_and_update_dependencies() {
        let mut m = Manifest::parse(SAMPLE).unwrap();
        let normal = DepTable::new(DepKind::Normal);

        m.set_dependency(&normal, &Dependency::new("rand", "0.9.2"));
        assert!(m.to_text().contains("rand = \"0.9.2\""));

        let mut serde = m.dependency(&normal, "serde").unwrap();
        serde.features.clear();
        m.set_dependency(&normal, &serde);
        // Nur noch die Version -> wieder Kurzform.
        assert!(m.to_text().contains("serde = \"1.0\""));

        let mut tokio = m.dependency(&normal, "tokio").unwrap();
        tokio.default_features = false;
        tokio.features = vec!["rt".into(), "macros".into()];
        tokio.optional = true;
        m.set_dependency(&normal, &tokio);
        let text = m.to_text();
        // Bleibt eine eigene Tabelle.
        assert!(
            text.contains("[dependencies.tokio]\nversion = \"1\"\nfeatures = [\"rt\", \"macros\"]")
        );
        assert!(text.contains("default-features = false"));
        assert!(text.contains("optional = true"));

        let reparsed = Manifest::parse(&text).unwrap();
        assert_eq!(reparsed.dependency(&normal, "tokio").unwrap(), tokio);

        assert!(m.remove_dependency(&normal, "anyhow"));
        assert!(!m.remove_dependency(&normal, "anyhow"));
    }

    #[test]
    fn sorted_tables_stay_sorted() {
        let mut m = Manifest::parse("[dependencies]\nanyhow = \"1\"\nzip = \"2\"\n").unwrap();
        m.set_dependency(
            &DepTable::new(DepKind::Normal),
            &Dependency::new("log", "0.4"),
        );
        assert_eq!(
            m.to_text(),
            "[dependencies]\nanyhow = \"1\"\nlog = \"0.4\"\nzip = \"2\"\n"
        );

        let mut m = Manifest::parse("[dependencies]\nzip = \"2\"\nanyhow = \"1\"\n").unwrap();
        m.set_dependency(
            &DepTable::new(DepKind::Normal),
            &Dependency::new("log", "0.4"),
        );
        assert!(m.to_text().ends_with("log = \"0.4\"\n"));
    }

    #[test]
    fn new_tables_are_created() {
        let mut m = Manifest::new_package("Mein Projekt");
        assert_eq!(
            m.package_field("name"),
            FieldValue::Text("mein-projekt".into())
        );
        m.set_dependency(
            &DepTable::new(DepKind::Dev),
            &Dependency::new("pretty_assertions", "1"),
        );
        m.set_dependency(
            &DepTable::for_target(DepKind::Normal, "cfg(windows)"),
            &Dependency::new("winapi", "0.3"),
        );
        let text = m.to_text();
        assert!(text.contains("[dev-dependencies]\npretty_assertions = \"1\""));
        assert!(text.contains("[target.'cfg(windows)'.dependencies]\nwinapi = \"0.3\""));
        assert!(!text.contains("[target]\n"));
    }

    #[test]
    fn workspace_and_renamed_dependencies() {
        let mut m = Manifest::parse("[dependencies]\nfoo = { workspace = true }\nbar = { package = \"real-bar\", version = \"2\" }\n").unwrap();
        let normal = DepTable::new(DepKind::Normal);
        let foo = m.dependency(&normal, "foo").unwrap();
        assert!(foo.workspace);
        assert_eq!(foo.summary(), "foo (workspace)");
        let mut bar = m.dependency(&normal, "bar").unwrap();
        assert_eq!(bar.crate_name(), "real-bar");
        bar.features.push("x".into());
        m.set_dependency(&normal, &bar);
        assert!(
            m.to_text()
                .contains("bar = { package = \"real-bar\", version = \"2\", features = [\"x\"] }")
        );
    }

    #[test]
    fn features_table() {
        let mut m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(m.features()[0], ("default".into(), vec!["std".into()]));
        m.set_feature("serde", &["dep:serde".into(), " ".into()]);
        assert!(m.to_text().contains("serde = [\"dep:serde\"]"));
        assert!(m.remove_feature("std"));
        assert_eq!(m.features().len(), 2);
    }

    #[test]
    fn sections() {
        let mut m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(
            m.sections(),
            ["package", "dependencies", "target", "features"]
        );
        assert!(m.section_text("features").starts_with("[features]\n"));

        m.set_section_text("features", "[features]\nfast = []\n")
            .unwrap();
        assert_eq!(m.features(), [("fast".to_owned(), vec![])]);
        // Position bleibt erhalten.
        assert_eq!(m.sections().last().unwrap(), "features");

        let err = m.set_section_text("features", "[features\n").unwrap_err();
        assert_eq!(err.line, 0);

        m.set_section_text(
            "profile",
            SECTION_TEMPLATES
                .iter()
                .find(|t| t.0 == "profile")
                .unwrap()
                .1,
        )
        .unwrap();
        assert!(m.to_text().contains("[profile.release]"));
        m.set_section_text("profile", "").unwrap();
        assert!(!m.sections().contains(&"profile".to_owned()));

        for (key, template) in SECTION_TEMPLATES {
            let mut m = Manifest::new_package("x");
            m.set_section_text(key, template).unwrap();
            assert!(Manifest::parse(&m.to_text()).is_ok(), "{key}");
            assert!(m.sections().contains(&key.to_string()), "{key}");
        }
    }

    #[test]
    fn helpers() {
        assert_eq!(split_list(" a, b ,,c "), ["a", "b", "c"]);
        assert_eq!(sanitize_package_name("123 Test"), "pkg-123-test");
    }
}
