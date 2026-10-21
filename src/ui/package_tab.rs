//! Tab 3: der `[package]`-Abschnitt als Formular.
//!
//! Jede Eingabe wird sofort ins Manifest übernommen (leere Felder werden
//! entfernt). Vom Workspace geerbte Felder (`version.workspace = true`) und
//! Sonderformen (z. B. `publish = ["registry"]`) sind hier nur sichtbar und
//! werden im Quelltext bearbeitet.

use std::rc::Rc;

use rncl::prelude::*;

use super::{GAP, MARGIN, Origin, ROW, Shared, State, weak1};
use crate::manifest::{FieldKind, FieldValue, PACKAGE_FIELDS, PackageField, split_list};
use crate::tr;

/// Das Eingabeelement eines Feldes.
#[derive(Debug, Clone)]
pub enum FieldEditor {
    Text(RnclTextField),
    Choice(RnclComboBox),
    Bool(RnclCheckBox),
}

impl FieldEditor {
    fn widget(&self) -> &Widget {
        match self {
            FieldEditor::Text(w) => w.widget(),
            FieldEditor::Choice(w) => w.widget(),
            FieldEditor::Bool(w) => w.widget(),
        }
    }

    /// Text-Eingabe (für Text- und Auswahlfelder).
    pub fn text(&self) -> String {
        match self {
            FieldEditor::Text(w) => w.text(),
            FieldEditor::Choice(w) => w.current_text(),
            FieldEditor::Bool(w) => w.is_checked().to_string(),
        }
    }

    pub fn set_text(&self, text: &str) {
        match self {
            FieldEditor::Text(w) => {
                if w.text() != text {
                    w.set_text(text);
                }
            }
            FieldEditor::Choice(w) => w.set_current_text(text),
            FieldEditor::Bool(_) => {}
        }
    }
}

/// Eine Zeile des Formulars.
#[derive(Debug, Clone)]
pub struct FieldRow {
    pub field: PackageField,
    pub label: RnclLabel,
    pub editor: FieldEditor,
}

pub struct PackageTab {
    pub page: RnclPanel,
    pub hint: RnclLabel,
    pub scroll: RnclScrollView,
    pub rows: Vec<FieldRow>,
}

impl PackageTab {
    pub fn build(page: &RnclPanel) -> Self {
        let hint = RnclLabel::new(page, &tr!("package.hint"));
        hint.set_word_wrap(true);
        let scroll = RnclScrollView::new(page);
        let content = scroll.content();

        let rows: Vec<FieldRow> = PACKAGE_FIELDS
            .iter()
            .map(|field| {
                let label = RnclLabel::new(content, field.key);
                label.set_tool_tip(&tr!(&format!("package.field.{}", field.key)));
                let editor = match field.kind {
                    FieldKind::Text | FieldKind::List => {
                        let edit = RnclTextField::new(content);
                        if field.kind == FieldKind::List {
                            edit.set_placeholder(&tr!("package.list_placeholder"));
                        }
                        FieldEditor::Text(edit)
                    }
                    FieldKind::Choice(values) => {
                        let combo = RnclComboBox::new(content);
                        combo.set_editable(true);
                        let mut items = vec![""];
                        items.extend_from_slice(values);
                        combo.set_items(&items);
                        FieldEditor::Choice(combo)
                    }
                    FieldKind::Bool { .. } => FieldEditor::Bool(RnclCheckBox::new(content, "")),
                };
                editor
                    .widget()
                    .set_tool_tip(&tr!(&format!("package.field.{}", field.key)));
                FieldRow {
                    field: *field,
                    label,
                    editor,
                }
            })
            .collect();
        scroll.set_content_size(0, 2 * GAP + rows.len() as i32 * (ROW + GAP));

        Self {
            page: page.clone(),
            hint,
            scroll,
            rows,
        }
    }

    pub fn layout(&self, w: i32, h: i32) {
        let hint_h = 40;
        self.hint
            .set_geometry(MARGIN, MARGIN, w - 2 * MARGIN, hint_h);
        let top = MARGIN + hint_h + GAP;
        self.scroll
            .set_geometry(MARGIN, top, w - 2 * MARGIN, h - top - MARGIN);
        let label_w = 140;
        let edit_w = (w - 2 * MARGIN - label_w - 3 * GAP - 30).clamp(200, 700);
        for (i, row) in self.rows.iter().enumerate() {
            let y = GAP + i as i32 * (ROW + GAP);
            row.label.set_geometry(GAP, y, label_w, ROW);
            row.editor
                .widget()
                .set_geometry(Rect::new(GAP + label_w + GAP, y, edit_w, ROW));
        }
    }

    pub fn refresh(&self, state: &State) {
        let manifest = &state.manifest;
        self.hint.set_text(&if manifest.has_package() {
            tr!("package.hint")
        } else {
            tr!("package.no_package")
        });
        for row in &self.rows {
            let value = manifest.package_field(row.field.key);
            let editable = matches!(
                (&value, row.field.kind),
                (
                    FieldValue::Missing | FieldValue::Text(_),
                    FieldKind::Text | FieldKind::Choice(_)
                ) | (FieldValue::Missing | FieldValue::List(_), FieldKind::List)
                    | (
                        FieldValue::Missing | FieldValue::Bool(_),
                        FieldKind::Bool { .. }
                    )
            );
            row.editor.widget().set_enabled(editable);
            match (&row.editor, value) {
                (FieldEditor::Bool(check), value) => {
                    let default = matches!(row.field.kind, FieldKind::Bool { default: true });
                    check.set_checked(match value {
                        FieldValue::Bool(b) => b,
                        _ => default,
                    });
                    check.set_text(&match value {
                        FieldValue::Inherited => tr!("package.inherited"),
                        FieldValue::Complex(raw) => raw,
                        _ => String::new(),
                    });
                }
                (editor, FieldValue::Missing) => editor.set_text(""),
                (editor, FieldValue::Text(t)) => editor.set_text(&t),
                (editor, FieldValue::List(items)) => editor.set_text(&items.join(", ")),
                (editor, FieldValue::Bool(b)) => editor.set_text(&b.to_string()),
                (editor, FieldValue::Inherited) => editor.set_text(&tr!("package.inherited")),
                (editor, FieldValue::Complex(raw)) => editor.set_text(&raw),
            }
        }
    }
}

fn apply_field(s: &Rc<Shared>, index: usize) {
    let row = &s.package.rows[index];
    let key = row.field.key;
    match (&row.editor, row.field.kind) {
        (FieldEditor::Bool(check), FieldKind::Bool { default }) => {
            let checked = check.is_checked();
            s.modify(Origin::Package, |m| {
                m.set_package_bool(key, checked, default)
            });
        }
        (editor, FieldKind::List) => {
            let items = split_list(&editor.text());
            s.modify(Origin::Package, |m| m.set_package_list(key, &items));
        }
        (editor, _) => {
            let text = editor.text();
            s.modify(Origin::Package, |m| m.set_package_text(key, &text));
        }
    }
}

pub fn connect(shared: &Rc<Shared>) {
    let weak = Rc::downgrade(shared);
    shared.package.page.widget().on_resized(move |w, h| {
        if let Some(s) = weak.upgrade() {
            s.package.layout(w, h);
        }
    });
    for (index, row) in shared.package.rows.iter().enumerate() {
        match &row.editor {
            FieldEditor::Text(edit) => {
                edit.on_text_changed(weak1(shared, move |s, _: String| apply_field(s, index)))
            }
            FieldEditor::Choice(combo) => {
                combo.on_text_changed(weak1(shared, move |s, _: String| apply_field(s, index)))
            }
            FieldEditor::Bool(check) => {
                check.on_toggled(weak1(shared, move |s, _: bool| apply_field(s, index)))
            }
        }
    }
}
