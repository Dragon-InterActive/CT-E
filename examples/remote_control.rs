//! Den Editor per Code fernsteuern – z. B. für Freya TD oder KI-Agenten.
//!
//! Sucht ein Crate auf crates.io, wählt den ersten Treffer, hakt ein
//! Feature an, trägt die Abhängigkeit ein und speichert ein Bildschirmfoto:
//!
//! ```bash
//! cargo run --example remote_control -- <projektordner> <suchbegriff> [feature] [bild.png] [sprache]
//! cargo run --example remote_control -- /tmp/demo serde derive /tmp/ct.png en
//! ```
//!
//! Linux ohne Bildschirm: `xvfb-run` voranstellen.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ct_editor::cli::Tab;
use ct_editor::crates_io::CratesIo;
use ct_editor::i18n;
use ct_editor::ui::{Editor, EditorOptions};
use rncl::prelude::*;

/// Arbeitet Events ab, bis `done()` gilt – Netzwerkantworten kommen asynchron.
fn wait_until(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "Zeitüberschreitung: {what}"
        );
        Application::process_events();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let project = args.next().unwrap_or_else(|| ".".into());
    let query = args.next().unwrap_or_else(|| "serde".into());
    let feature = args.next();
    let screenshot = args.next().unwrap_or_else(|| "ct-editor.png".into());
    let language = args.next().unwrap_or_else(|| "de".into());

    i18n::set_language(&language);
    let _app = Application::new();
    let editor = Editor::open(EditorOptions {
        project_dir: project.into(),
        source: Arc::new(CratesIo::new()),
        size: (1100, 720),
    });
    editor.select_tab(Tab::Dependencies);
    editor.show();

    let deps = &editor.shared().deps;
    deps.search.set_text(&query);
    wait_until("Suchergebnisse", || deps.results.count() > 0);
    deps.results.set_current_index(0);
    wait_until("Crate-Details", || deps.apply.is_enabled());

    if let Some(feature) = feature {
        let data = deps.data.borrow();
        match data.states.iter().position(|s| s.name == feature) {
            Some(i) => data.checkboxes[i].set_checked(true),
            None => eprintln!("Feature {feature} gibt es nicht"),
        }
    }
    deps.apply.click();
    Application::process_events();

    println!("{}", editor.text());
    editor.window().widget().save_screenshot(&screenshot);
    println!("Bild: {screenshot}");
}
