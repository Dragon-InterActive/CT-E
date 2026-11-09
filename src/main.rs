//! Startpunkt: `ct-editor --lang de --path <projektordner>`.

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use ct_editor::cli::Cli;
use ct_editor::crates_io::{CrateSource, CratesIo, StaticSource};
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
    let cli = Cli::parse();

    if let Some(dir) = &cli.locales
        && let Err(error) = i18n::load_dir(dir)
    {
        eprintln!("ct-editor: {error}");
    }
    i18n::set_language(&cli.language());

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
    let editor = Editor::open(EditorOptions {
        project_dir: cli.project_dir(),
        source,
        size: cli.size,
    });
    editor.select_tab(cli.tab);
    editor.show();

    if let Some(path) = &cli.screenshot {
        for _ in 0..5 {
            Application::process_events();
        }
        return if editor.window().widget().save_screenshot(path) {
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
    drop(editor);
    ExitCode::from(code.clamp(0, 255) as u8)
}
