//! Tab 5: "Erweitert" – jeder Abschnitt der `Cargo.toml` einzeln als TOML.
//!
//! Damit ist der volle Umfang des Manifests visuell erreichbar, auch das,
//! wofür es kein eigenes Formular gibt: `[lib]`, `[[bin]]`, `[[example]]`,
//! `[[test]]`, `[[bench]]`, `[profile.*]`, `[workspace]`, `[lints]`,
//! `[patch]`, `[replace]`, `[badges]`, `[target.*]`, `[package.metadata]` …
//! Neue Abschnitte entstehen aus Vorlagen.

use std::cell::RefCell;
use std::rc::Rc;

use rncl::prelude::*;

use super::source_tab::TOML_RULES;
use super::{GAP, MARGIN, Origin, ROW, Shared, State, StatusKind, weak0, weak1};
use crate::manifest::SECTION_TEMPLATES;
use crate::tr;

pub struct AdvancedTab {
    pub page: RnclPanel,
    pub section_label: RnclLabel,
    pub section: RnclComboBox,
    pub template_label: RnclLabel,
    pub template: RnclComboBox,
    pub insert: RnclButton,
    pub editor: RnclTextArea,
    pub apply: RnclButton,
    pub remove: RnclButton,
    pub error: RnclLabel,
    /// Der Abschnitt, auf den sich der Editor bezieht (auch ein noch neuer).
    pub current: RefCell<Option<String>>,
}

impl AdvancedTab {
    pub fn build(page: &RnclPanel) -> Self {
        let section_label = RnclLabel::new(page, &tr!("advanced.section"));
        let section = RnclComboBox::new(page);
        let template_label = RnclLabel::new(page, &tr!("advanced.template"));
        let template = RnclComboBox::new(page);
        template.set_items(
            &SECTION_TEMPLATES
                .iter()
                .map(|(k, _)| *k)
                .collect::<Vec<_>>(),
        );
        let insert = RnclButton::new(page, &tr!("advanced.insert"));
        let editor = RnclTextArea::new(page);
        editor.set_monospace(true);
        for (pattern, color, bold) in TOML_RULES {
            editor.add_highlight_rule(pattern, color, *bold);
        }
        let apply = RnclButton::new(page, &tr!("advanced.apply"));
        let remove = RnclButton::new(page, &tr!("advanced.remove"));
        let error = RnclLabel::new(page, "");
        error.set_style_sheet("color: #c0392b;");
        Self {
            page: page.clone(),
            section_label,
            section,
            template_label,
            template,
            insert,
            editor,
            apply,
            remove,
            error,
            current: RefCell::new(None),
        }
    }

    pub fn layout(&self, w: i32, h: i32) {
        let label_w = 90;
        let y = MARGIN;
        self.section_label.set_geometry(MARGIN, y, label_w, ROW);
        self.section.set_geometry(MARGIN + label_w, y, 200, ROW);
        let x = MARGIN + label_w + 200 + 3 * GAP;
        self.template_label.set_geometry(x, y, 120, ROW);
        self.template.set_geometry(x + 120, y, 160, ROW);
        self.insert.set_geometry(x + 120 + 160 + GAP, y, 150, ROW);

        let top = y + ROW + GAP;
        let bottom_h = ROW + GAP + 22;
        self.editor
            .set_geometry(MARGIN, top, w - 2 * MARGIN, h - top - MARGIN - bottom_h);
        let by = h - MARGIN - ROW - 22 - GAP;
        self.apply.set_geometry(MARGIN, by, 150, ROW);
        self.remove.set_geometry(MARGIN + 150 + GAP, by, 150, ROW);
        self.error
            .set_geometry(MARGIN, h - MARGIN - 22, w - 2 * MARGIN, 22);
    }

    pub fn refresh(&self, state: &State) {
        let sections = state.manifest.sections();
        let current = self.current.borrow().clone();
        if self.section.items() != sections {
            self.section.set_items(&sections);
        }
        let current = current
            .filter(|c| sections.contains(c))
            .or_else(|| sections.first().cloned());
        if let Some(key) = &current {
            self.section.set_current_text(key);
            self.editor.set_text(&state.manifest.section_text(key));
        } else {
            self.editor.set_text("");
        }
        *self.current.borrow_mut() = current;
        self.editor.set_marked_line(None, "");
        self.error.set_text("");
    }
}

pub fn connect(shared: &Rc<Shared>) {
    let tab = &shared.advanced;
    let weak = Rc::downgrade(shared);
    tab.page.widget().on_resized(move |w, h| {
        if let Some(s) = weak.upgrade() {
            s.advanced.layout(w, h);
        }
    });

    tab.section
        .on_current_index_changed(weak1(shared, |s, _: i32| {
            let tab = &s.advanced;
            let key = tab.section.current_text();
            *tab.current.borrow_mut() = (!key.is_empty()).then_some(key.clone());
            let text = s.state.borrow().manifest.section_text(&key);
            tab.editor.set_text(&text);
            tab.editor.set_marked_line(None, "");
            tab.error.set_text("");
        }));

    tab.insert.on_clicked(weak0(shared, |s| {
        let tab = &s.advanced;
        let index = tab.template.current_index().max(0) as usize;
        let Some((key, template)) = SECTION_TEMPLATES.get(index) else {
            return;
        };
        let existing = s.state.borrow().manifest.section_text(key);
        // Gibt es den Abschnitt schon, wird die Vorlage angehängt
        // (z. B. ein weiteres [[bin]] oder [profile.dev] neben [profile.release]).
        let text = if existing.is_empty() {
            template.to_string()
        } else {
            format!("{}\n\n{template}", existing.trim_end())
        };
        *tab.current.borrow_mut() = Some(key.to_string());
        tab.editor.set_text(&text);
        s.set_status(
            StatusKind::Info,
            &tr!("advanced.template_inserted", section = key),
        );
    }));

    tab.apply.on_clicked(weak0(shared, |s| {
        let tab = &s.advanced;
        let Some(key) = tab.current.borrow().clone() else {
            return;
        };
        let text = tab.editor.text();
        match s.try_modify(Origin::Advanced, |m| m.set_section_text(&key, &text)) {
            Ok(()) => {
                s.quietly(|state| tab.refresh(state));
                s.set_status(StatusKind::Success, &tr!("advanced.applied", section = key));
            }
            Err(error) => {
                tab.editor
                    .set_marked_line(Some(error.line as i32), "#f5b7b1");
                tab.error.set_text(&tr!(
                    "source.error",
                    line = error.line + 1,
                    column = error.column + 1,
                    message = error.message
                ));
            }
        }
    }));

    tab.remove.on_clicked(weak0(shared, |s| {
        let tab = &s.advanced;
        let Some(key) = tab.current.borrow().clone() else {
            return;
        };
        let mut removed = false;
        s.modify(Origin::Advanced, |m| removed = m.remove_section(&key));
        *tab.current.borrow_mut() = None;
        s.quietly(|state| tab.refresh(state));
        if removed {
            s.set_status(StatusKind::Success, &tr!("advanced.removed", section = key));
        }
    }));
}
