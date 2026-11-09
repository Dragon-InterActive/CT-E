//! Integrationstests: der Editor wird wie von einem Anwender bedient –
//! gegen das echte native Toolkit und eine feste Crate-Quelle statt crates.io.
//!
//! Native Toolkits erlauben nur eine Anwendung pro Prozess, im Hauptthread –
//! daher `harness = false` und ein eigenes `main()`. Linux ohne Bildschirm:
//! `xvfb-run cargo test`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ct_editor::cli::Tab;
use ct_editor::crates_io::{CrateDetails, CrateVersion, StaticSource};
use ct_editor::i18n;
use ct_editor::ui::{Editor, EditorOptions};
use rncl::prelude::*;

fn main() {
    let app = Application::new();
    i18n::set_language("de");

    let tests: &[(&str, fn())] = &[
        (
            "new_manifest_is_created_and_saved",
            new_manifest_is_created_and_saved,
        ),
        (
            "search_needs_three_characters",
            search_needs_three_characters,
        ),
        ("add_dependency_with_features", add_dependency_with_features),
        ("deselecting_defaults", deselecting_defaults),
        ("edit_and_remove_existing", edit_and_remove_existing),
        (
            "syntax_error_locks_visual_tabs",
            syntax_error_locks_visual_tabs,
        ),
        ("package_form", package_form),
        ("features_tab", features_tab),
        ("advanced_tab", advanced_tab),
        ("target_and_dev_tables", target_and_dev_tables),
        (
            "existing_file_is_loaded_unchanged",
            existing_file_is_loaded_unchanged,
        ),
        ("standalone_menu", standalone_menu),
    ];
    for (name, test) in tests {
        test();
        println!("test {name} ... ok");
    }
    println!("\ntest result: ok. {} passed", tests.len());
    drop(app);
}

// --- Hilfen -----------------------------------------------------------------

fn wait_until(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Zeitüberschreitung: {what}"
        );
        Application::process_events();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ct-editor-gui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn version(num: &str, features: &[(&str, &[&str])]) -> CrateVersion {
    CrateVersion {
        num: num.into(),
        yanked: false,
        rust_version: None,
        features: features
            .iter()
            .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn source() -> Arc<StaticSource> {
    let serde_features: &[(&str, &[&str])] = &[
        ("default", &["std"]),
        ("std", &[]),
        ("derive", &["dep:serde_derive"]),
        ("rc", &[]),
    ];
    Arc::new(StaticSource {
        crates: vec![
            CrateDetails {
                name: "serde".into(),
                description: Some("A generic serialization framework".into()),
                homepage: None,
                repository: Some("https://github.com/serde-rs/serde".into()),
                documentation: None,
                default_version: Some("1.0.229".into()),
                versions: vec![
                    version("1.0.229", serde_features),
                    version("0.9.15", &[("std", &[])]),
                ],
            },
            CrateDetails {
                name: "serde_json".into(),
                description: Some("JSON".into()),
                homepage: None,
                repository: None,
                documentation: None,
                default_version: Some("1.0.154".into()),
                versions: vec![version("1.0.154", &[("default", &["std"]), ("std", &[])])],
            },
        ],
    })
}

fn open(dir: &Path) -> Editor {
    let editor = Editor::open(EditorOptions {
        project_dir: dir.to_path_buf(),
        source: source(),
        size: (1100, 720),
        menu: false,
    });
    editor.show();
    Application::process_events();
    editor
}

/// Sucht und klickt den ersten Treffer an; wartet, bis die Features da sind.
fn search_and_pick(editor: &Editor, query: &str) {
    let deps = &editor.shared().deps;
    deps.search.set_text(query);
    wait_until("Suchergebnisse", || deps.results.count() > 0);
    deps.results.set_current_index(0);
    wait_until("Crate-Details", || deps.apply.is_enabled());
}

fn feature_box(editor: &Editor, name: &str) -> RnclCheckBox {
    let data = editor.shared().deps.data.borrow();
    let index = data.states.iter().position(|s| s.name == name).expect(name);
    data.checkboxes[index].clone()
}

// --- Tests ------------------------------------------------------------------

fn new_manifest_is_created_and_saved() {
    let dir = temp_dir("Neues Projekt");
    let editor = open(&dir);
    assert!(!editor.manifest_path().exists());
    let text = editor.text();
    assert!(
        text.contains("name = \"ct-editor-gui-neues-projekt-"),
        "{text}"
    );
    assert!(text.contains("[dependencies]"));
    assert!(editor.is_dirty(), "neue Datei gilt als ungespeichert");

    let path = editor.save().unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert!(!editor.is_dirty());
    assert!(editor.window().title().ends_with("Cargo.toml"));
    std::fs::remove_dir_all(dir).ok();
}

fn search_needs_three_characters() {
    let dir = temp_dir("min3");
    let editor = open(&dir);
    let deps = &editor.shared().deps;
    deps.search.set_text("se");
    for _ in 0..30 {
        Application::process_events();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        deps.results.count(),
        0,
        "unter 3 Zeichen wird nicht gesucht"
    );
    assert!(!deps.debounce.is_active());

    deps.search.set_text("ser");
    assert!(
        deps.debounce.is_active(),
        "ab 3 Zeichen startet die Suche nach der Pause"
    );
    wait_until("Treffer", || deps.results.count() == 2);
    assert_eq!(
        deps.results.items(),
        ["serde (1.0.229)", "serde_json (1.0.154)"]
    );

    // Zurück unter 3 Zeichen: Liste leer, laufende Suche verworfen.
    deps.search.set_text("s");
    assert_eq!(deps.results.count(), 0);
}

fn add_dependency_with_features() {
    let dir = temp_dir("add");
    let editor = open(&dir);
    search_and_pick(&editor, "serde");
    let deps = &editor.shared().deps;

    // Standard: die Default-Features des Crates sind angehakt.
    assert!(feature_box(&editor, "std").is_checked());
    assert!(!feature_box(&editor, "derive").is_checked());
    assert_eq!(deps.version.current_text(), "1.0.229");
    assert!(
        deps.info
            .text()
            .contains("A generic serialization framework")
    );

    // Ohne Änderung: Kurzform.
    deps.apply.click();
    assert!(
        editor.text().contains("serde = \"1.0.229\""),
        "{}",
        editor.text()
    );

    // Zusätzliches Feature.
    feature_box(&editor, "derive").set_checked(true);
    deps.apply.click();
    assert!(
        editor
            .text()
            .contains("serde = { version = \"1.0.229\", features = [\"derive\"] }"),
        "{}",
        editor.text()
    );
    assert_eq!(deps.list.items(), ["serde 1.0.229 [derive]"]);
    assert_eq!(deps.apply.text(), "Übernehmen");
    // Der Quelltext-Tab zeigt dasselbe.
    assert_eq!(editor.shared().source.editor.text(), editor.text());
}

fn deselecting_defaults() {
    let dir = temp_dir("nodefault");
    let editor = open(&dir);
    search_and_pick(&editor, "serde");
    feature_box(&editor, "std").set_checked(false);
    feature_box(&editor, "rc").set_checked(true);
    editor.shared().deps.apply.click();
    assert!(
        editor.text().contains(
            "serde = { version = \"1.0.229\", default-features = false, features = [\"rc\"] }"
        ),
        "{}",
        editor.text()
    );
}

fn edit_and_remove_existing() {
    let dir = temp_dir("existing");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = { version = \"1.0\", features = [\"rc\"] } # wichtig\n",
    )
    .unwrap();
    let editor = open(&dir);
    assert!(!editor.is_dirty());
    let deps = &editor.shared().deps;
    assert_eq!(deps.list.items(), ["serde 1.0 [rc]"]);

    deps.list.set_current_index(0);
    wait_until("Details der vorhandenen Abhängigkeit", || {
        deps.apply.is_enabled()
    });
    // Häkchen kommen aus dem Manifest: Defaults + rc.
    assert!(feature_box(&editor, "std").is_checked());
    assert!(feature_box(&editor, "rc").is_checked());
    assert_eq!(deps.version.current_text(), "1.0");
    assert!(deps.remove.is_enabled());

    feature_box(&editor, "derive").set_checked(true);
    deps.apply.click();
    assert!(
        editor
            .text()
            .contains("serde = { version = \"1.0\", features = [\"rc\", \"derive\"] } # wichtig"),
        "Kommentar bleibt erhalten: {}",
        editor.text()
    );
    assert!(editor.is_dirty());

    deps.remove.click();
    assert!(!editor.text().contains("serde"));
    assert_eq!(deps.list.count(), 0);
    std::fs::remove_dir_all(dir).ok();
}

fn syntax_error_locks_visual_tabs() {
    let dir = temp_dir("syntax");
    let editor = open(&dir);
    let shared = editor.shared();
    editor.set_source_text("[package]\nname = \"x\nversion = \"0.1.0\"\n");
    let error = editor.parse_error().expect("Syntaxfehler erkannt");
    assert_eq!(error.line, 1);
    assert!(!shared.deps.page.is_enabled(), "visuelle Tabs gesperrt");
    assert!(!shared.package.page.is_enabled());
    assert!(shared.status.text().contains("Zeile 2"));
    assert!(shared.source.error.text().contains("Zeile 2"));
    assert!(
        editor.save().is_err(),
        "mit Syntaxfehler wird nicht gespeichert"
    );

    editor.set_source_text("[package]\nname = \"x\"\nversion = \"0.1.0\"\n");
    assert!(editor.parse_error().is_none());
    assert!(shared.deps.page.is_enabled());
    assert_eq!(shared.source.error.text(), "");
    assert!(shared.status.text().contains("Syntax in Ordnung"));

    // Tipp-Pause: der Text wird erst nach dem Timer ausgewertet.
    shared.source.editor.set_text("[package]\nname = \"y\"\n");
    assert!(shared.source.debounce.is_active());
    wait_until("Auswertung nach der Pause", || {
        editor.text().contains("\"y\"")
    });
}

fn package_form() {
    let dir = temp_dir("package");
    let editor = open(&dir);
    editor.select_tab(Tab::Package);
    let package = &editor.shared().package;
    let row = |key: &str| {
        package
            .rows
            .iter()
            .find(|r| r.field.key == key)
            .unwrap()
            .clone()
    };

    let ctor = |key: &str| row(key).editor;
    ctor("description").set_text("Mein Projekt");
    assert!(editor.text().contains("description = \"Mein Projekt\""));
    ctor("keywords").set_text("gui, qt , ");
    assert!(editor.text().contains("keywords = [\"gui\", \"qt\"]"));
    ctor("edition").set_text("2021");
    assert!(editor.text().contains("edition = \"2021\""));
    ctor("description").set_text("");
    assert!(!editor.text().contains("description"));
    if let ct_editor::ui::package_tab::FieldEditor::Bool(check) = ctor("publish") {
        assert!(check.is_checked(), "Standard: publish = true");
        check.set_checked(false);
    }
    assert!(editor.text().contains("publish = false"));

    // Änderung im Quelltext erscheint im Formular.
    let text = editor
        .text()
        .replace("edition = \"2021\"", "edition = \"2018\"");
    editor.set_source_text(&text);
    assert_eq!(ctor("edition").text(), "2018");
}

fn features_tab() {
    let dir = temp_dir("features");
    let editor = open(&dir);
    let features = &editor.shared().features;
    features.name.set_text("std");
    features.entries.set_text("");
    features.apply.click();
    features.new.click();
    features.name.set_text("default");
    features.entries.set_text("std");
    features.apply.click();
    assert!(
        editor
            .text()
            .contains("[features]\nstd = []\ndefault = [\"std\"]"),
        "{}",
        editor.text()
    );
    assert_eq!(features.list.items(), ["std = []", "default = [std]"]);

    // Umbenennen über die Liste.
    features.list.set_current_index(0);
    assert_eq!(features.name.text(), "std");
    features.name.set_text("alloc");
    features.apply.click();
    assert!(editor.text().contains("alloc = []"));
    assert!(!editor.text().contains("std = []"));

    features.name.set_text("alloc");
    features.remove.click();
    assert!(!editor.text().contains("alloc"));
}

fn advanced_tab() {
    let dir = temp_dir("advanced");
    let editor = open(&dir);
    let advanced = &editor.shared().advanced;
    assert_eq!(advanced.section.items(), ["package", "dependencies"]);

    let profile = ct_editor::manifest::SECTION_TEMPLATES
        .iter()
        .position(|(k, _)| *k == "profile")
        .unwrap();
    advanced.template.set_current_index(profile as i32);
    advanced.insert.click();
    advanced.apply.click();
    assert!(
        editor.text().contains("[profile.release]\nopt-level = 3"),
        "{}",
        editor.text()
    );
    assert!(advanced.section.items().contains(&"profile".to_owned()));

    // Fehlerhaftes TOML: nicht übernommen, Zeile markiert.
    advanced.editor.set_text("[profile.release\n");
    advanced.apply.click();
    assert!(advanced.error.text().contains("Zeile 1"));
    assert!(editor.text().contains("[profile.release]\nopt-level = 3"));

    advanced.section.set_current_text("profile");
    advanced.remove.click();
    assert!(!editor.text().contains("profile"));
}

fn target_and_dev_tables() {
    let dir = temp_dir("tables");
    let editor = open(&dir);
    let deps = &editor.shared().deps;
    deps.kind.set_current_index(1); // dev-dependencies
    search_and_pick(&editor, "serde_json");
    deps.apply.click();
    assert!(
        editor
            .text()
            .contains("[dev-dependencies]\nserde_json = \"1.0.154\""),
        "{}",
        editor.text()
    );

    deps.kind.set_current_index(0);
    deps.target.set_current_text("cfg(unix)");
    search_and_pick(&editor, "serde_json");
    deps.apply.click();
    assert!(
        editor
            .text()
            .contains("[target.'cfg(unix)'.dependencies]\nserde_json = \"1.0.154\""),
        "{}",
        editor.text()
    );
    assert_eq!(
        deps.list_label.text(),
        "Abhängigkeiten in [target.'cfg(unix)'.dependencies]:"
    );
}

fn existing_file_is_loaded_unchanged() {
    let dir = temp_dir("roundtrip");
    std::fs::create_dir_all(&dir).unwrap();
    let original = "# Kopfkommentar\n[package]\nname   =   \"spaces\"   # bleibt so\nversion = \"1.0.0\"\n\n[dependencies]\n";
    std::fs::write(dir.join("Cargo.toml"), original).unwrap();
    let editor = open(&dir);
    assert_eq!(editor.text(), original);
    assert!(!editor.is_dirty());
    editor.select_tab(Tab::Advanced);
    editor.select_tab(Tab::Source);
    assert_eq!(editor.text(), original, "Tabwechsel ändert nichts");
    std::fs::remove_dir_all(dir).ok();
}

/// Eigenständig mit Menü „Datei“: Sprache wechseln (Text bleibt erhalten),
/// anderes Projekt öffnen.
fn standalone_menu() {
    use ct_editor::ui::Request;
    use ct_editor::ui::standalone::Standalone;
    let dir = temp_dir("standalone");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"eins\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let other = temp_dir("standalone-other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(
        other.join("Cargo.toml"),
        "[package]\nname = \"drei\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let frame = Standalone::start(&dir, source(), (1100, 720), false);
    Application::process_events();
    assert_eq!(frame.editor().shared().save_button.text(), "Speichern");
    frame.editor().set_current_tab(Tab::Package.index());
    let edited = "[package]\nname = \"zwei\"\nversion = \"0.1.0\"\n";
    frame.editor().set_source_text(edited);
    assert!(frame.editor().is_dirty());

    // Menü Datei → Sprache → English: neu aufgebaut, Text und Tab bleiben.
    frame
        .editor()
        .window()
        .widget()
        .activate_menu_item("lang.en");
    wait_until("englische Oberfläche", || {
        Application::process_events();
        frame.editor().shared().save_button.text() == "Save"
    });
    assert_eq!(frame.editor().text(), edited);
    assert!(frame.editor().is_dirty());
    assert_eq!(frame.editor().current_tab(), Tab::Package.index());

    // Anderes Projekt öffnen (der Dateidialog liefert sonst den Ordner).
    frame.handle(Request::Open(other.clone()));
    Application::process_events();
    assert_eq!(frame.editor().manifest_path(), other.join("Cargo.toml"));
    assert!(!frame.editor().is_dirty());
    i18n::set_language("de");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}
