//! Startpunkt: `ct-editor --lang de --path <projektordner>`.

// Unter Windows ein GUI-Programm ohne eigenes Konsolenfenster (Release-Builds);
// aus einer Eingabeaufforderung gestartet, schreibt es in deren Konsole.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use ct_editor::cli::Cli;
use ct_editor::crates_io::{CrateSource, CratesIo, StaticSource};
use ct_editor::ui::standalone::Standalone;
use ct_editor::ui::{Editor, EditorOptions};
use ct_editor::{i18n, tr};
use rncl::prelude::*;

/// Programmsymbol in mehreren Größen (Quelle `assets/ct-editor.svg`).
const ICONS: [(u32, &[u8]); 7] = [
    (16, include_bytes!("../assets/icons/ct-editor-16.png")),
    (24, include_bytes!("../assets/icons/ct-editor-24.png")),
    (32, include_bytes!("../assets/icons/ct-editor-32.png")),
    (48, include_bytes!("../assets/icons/ct-editor-48.png")),
    (64, include_bytes!("../assets/icons/ct-editor-64.png")),
    (128, include_bytes!("../assets/icons/ct-editor-128.png")),
    (256, include_bytes!("../assets/icons/ct-editor-256.png")),
];

fn main() -> ExitCode {
    attach_parent_console();
    let cli = Cli::parse();

    if let Some(dir) = &cli.locales
        && let Err(error) = i18n::load_dir(dir)
    {
        eprintln!("ct-editor: {error}");
    }
    // Eigenständig: gespeicherte Sprachwahl (zwischen --lang und LANG).
    let saved = (!cli.embedded)
        .then(|| ct_editor::settings::Settings::load().language)
        .flatten();
    i18n::set_language(&cli.language_with(saved.as_deref()));

    // Screenshots ohne Bildschirm: unter Linux mit `xvfb-run` starten.
    let app = Application::new();
    app.set_name("CT-Editor");
    // Unter Wayland übergibt GTK das Symbol per xdg-toplevel-icon (KWin, GNOME
    // …), unter X11 als _NET_WM_ICON; ohne Symbol zeigt Wayland ein „W“.
    Application::set_icon("ct-editor", &ICONS);

    let source: Arc<dyn CrateSource> = if cli.offline {
        Arc::new(StaticSource::default())
    } else {
        Arc::new(CratesIo::new())
    };
    // Von Freya TDE gestartet: ohne Menü; sonst eigenständig mit Menü „Datei“.
    let embedded = cli.embedded.then(|| {
        Editor::open(EditorOptions {
            project_dir: cli.project_dir(),
            source: Arc::clone(&source),
            size: cli.size,
            menu: false,
        })
    });
    let standalone = (!cli.embedded).then(|| {
        Standalone::start(
            &cli.project_dir(),
            Arc::clone(&source),
            cli.size,
            cli.screenshot.is_none(),
        )
    });
    let window = match (&embedded, &standalone) {
        (Some(editor), _) => {
            editor.select_tab(cli.tab);
            editor.show();
            editor.window().widget().clone()
        }
        (None, Some(standalone)) => {
            standalone.editor().select_tab(cli.tab);
            standalone.editor().window().widget().clone()
        }
        (None, None) => unreachable!("eingebettet oder eigenständig"),
    };

    if let Some(path) = &cli.screenshot {
        for _ in 0..5 {
            Application::process_events();
        }
        return if window.save_screenshot(path) {
            ExitCode::SUCCESS
        } else {
            eprintln!(
                "ct-editor: {}",
                tr!("app.screenshot_failed", path = path.display())
            );
            ExitCode::FAILURE
        };
    }

    let code = app.exec();
    drop(embedded);
    drop(standalone);
    ExitCode::from(code.clamp(0, 255) as u8)
}

/// Windows (Release): Ausgaben wie `--help` und Fehlermeldungen in die Konsole
/// schreiben, aus der das Programm gestartet wurde (ohne Konsole: nichts).
fn attach_parent_console() {
    #[cfg(all(windows, not(debug_assertions)))]
    // SAFETY: einfacher Systemaufruf ohne Zeiger, ganz am Programmanfang.
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(
            windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
        );
    }
}
