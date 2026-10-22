//! Startpunkt: `ct-editor --lang de --path <projektordner>`.

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use ct_editor::cli::Cli;
use ct_editor::crates_io::{CrateSource, CratesIo, StaticSource};
use ct_editor::ui::{Editor, EditorOptions};
use ct_editor::{i18n, tr};
use rncl::prelude::*;

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
