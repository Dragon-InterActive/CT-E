//! Tab 1: der Quelltext der `Cargo.toml`.
//!
//! Syntax-Hervorhebung, Prüfung nach jeder Tipp-Pause (300 ms) und farbige
//! Markierung der Fehlerzeile. Erst wenn der Text gültig ist, übernehmen ihn
//! die visuellen Tabs.

use std::rc::Rc;

use rncl::prelude::*;
use ruf::Timer;

use super::{GAP, MARGIN, Origin, Shared, State, weak0, weak1};
use crate::manifest::ParseError;
use crate::tr;

/// Wartezeit nach der letzten Eingabe, bevor geprüft wird.
pub const DEBOUNCE_MS: i32 = 300;

/// Hervorhebungsregeln für TOML: (Muster, Farbe, fett). Spätere Regeln
/// überschreiben frühere – Kommentare stehen deshalb am Ende.
pub const TOML_RULES: &[(&str, &str, bool)] = &[
    (r"^\s*[A-Za-z0-9_\-\.'\x22]+(?=\s*=)", "#2874a6", false),
    (r"\b(true|false)\b", "#ca6f1e", false),
    (r"(?<![\w\.\-])[+-]?\d[\d_]*(\.\d+)?\b", "#ca6f1e", false),
    (r#""(?:[^"\\]|\\.)*""#, "#239b56", false),
    (r"'[^']*'", "#239b56", false),
    (r"^\s*\[\[?[^\]]*\]\]?", "#8e44ad", true),
    (r"#.*$", "#7f8c8d", false),
];

pub struct SourceTab {
    pub page: RnclPanel,
    pub editor: RnclTextArea,
    pub error: RnclLabel,
    pub debounce: Timer,
}

impl SourceTab {
    pub fn build(page: &RnclPanel) -> Self {
        let editor = RnclTextArea::new(page);
        editor.set_monospace(true);
        for (pattern, color, bold) in TOML_RULES {
            editor.add_highlight_rule(pattern, color, *bold);
        }
        let error = RnclLabel::new(page, "");
        error.set_style_sheet("color: #c0392b;");
        let debounce = Timer::new();
        debounce.set_single_shot(true);

        let (e, l) = (editor.clone(), error.clone());
        page.widget().on_resized(move |w, h| {
            let error_h = 22;
            e.set_geometry(
                MARGIN,
                MARGIN,
                w - 2 * MARGIN,
                h - 2 * MARGIN - error_h - GAP,
            );
            l.set_geometry(MARGIN, h - MARGIN - error_h, w - 2 * MARGIN, error_h);
        });

        Self {
            page: page.clone(),
            editor,
            error,
            debounce,
        }
    }

    pub fn refresh(&self, state: &State) {
        if self.editor.text() != state.text {
            self.editor.set_text(&state.text);
        }
    }

    pub fn show_error(&self, error: Option<&ParseError>) {
        match error {
            Some(e) => {
                self.editor.set_marked_line(Some(e.line as i32), "#f5b7b1");
                self.error.set_text(&tr!(
                    "source.error",
                    line = e.line + 1,
                    column = e.column + 1,
                    message = e.message
                ));
            }
            None => {
                self.editor.set_marked_line(None, "");
                self.error.set_text("");
            }
        }
    }
}

pub fn connect(shared: &Rc<Shared>) {
    let tab = &shared.source;
    tab.editor
        .on_text_changed(weak1(shared, |s, _text: String| {
            s.source.debounce.start(DEBOUNCE_MS);
        }));
    tab.debounce.on_timeout(weak0(shared, |s| {
        let text = s.source.editor.text();
        s.apply_source_text(&text, Origin::Source);
    }));
    // Klick auf die Fehlermeldung ist nicht möglich (Label) – F2 springt hin.
    shared.window.add_shortcut(
        "F2",
        weak0(shared, |s| {
            let line = s.state.borrow().parse_error.as_ref().map(|e| e.line as i32);
            if let Some(line) = line {
                s.tabs.set_current_index(0);
                s.source.editor.goto_line(line);
            }
        }),
    );
}
