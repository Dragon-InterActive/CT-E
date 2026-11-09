//! Der eigenständige Betrieb: ein [`Editor`] mit Menü „Datei“, den dieser
//! Rahmen bei „Öffnen“ und „Sprache“ neu aufbaut – an derselben Stelle, mit
//! derselben Registerkarte; beim Sprachwechsel bleibt ungespeicherter Text
//! erhalten. Die gewählte Sprache wird gespeichert ([`Settings`]).
//!
//! Von Freya TDE gestartet (`--embedded`) läuft der Editor ohne diesen Rahmen.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use rncl::prelude::*;

use super::{Editor, EditorOptions, Request};
use crate::crates_io::CrateSource;
use crate::i18n;
use crate::settings::Settings;

/// Der Rahmen des eigenständigen Editors.
pub struct Standalone {
    editor: RefCell<Option<Editor>>,
    /// Abgelöste Editoren; ihre Fenster schließen nach dem Wechsel.
    retired: RefCell<Vec<Editor>>,
    source: Arc<dyn CrateSource>,
    size: (i32, i32),
    /// Sprachwahl speichern (aus in Tests).
    remember_language: bool,
    me: Weak<Standalone>,
}

impl Standalone {
    /// Öffnet `Cargo.toml` in `project_dir` mit Menü und zeigt das Fenster.
    pub fn start(
        project_dir: &Path,
        source: Arc<dyn CrateSource>,
        size: (i32, i32),
        remember_language: bool,
    ) -> Rc<Self> {
        let this = Rc::new_cyclic(|me| Self {
            editor: RefCell::new(None),
            retired: RefCell::new(Vec::new()),
            source,
            size,
            remember_language,
            me: me.clone(),
        });
        this.open(project_dir, None);
        this
    }

    /// Der aktuelle Editor.
    pub fn editor(&self) -> std::cell::Ref<'_, Editor> {
        std::cell::Ref::map(self.editor.borrow(), |e| e.as_ref().expect("Editor"))
    }

    /// Führt eine Anforderung aus dem Menü aus (auch für Tests und Automatisierung).
    pub fn handle(&self, request: Request) {
        match request {
            Request::Open(dir) => self.open(&dir, None),
            Request::Language(language) => {
                let chosen = i18n::set_language(&language);
                if self.remember_language {
                    let settings = Settings {
                        language: Some(chosen),
                    };
                    if let Err(error) = settings.save() {
                        eprintln!("ct-editor: {error}");
                    }
                }
                // Gleicher Ordner; geänderter Text kommt mit.
                let (dir, text) = {
                    let editor = self.editor();
                    let dir = editor
                        .manifest_path()
                        .parent()
                        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
                    (dir, editor.is_dirty().then(|| editor.text()))
                };
                self.open(&dir, text);
            }
        }
    }

    /// Baut den Editor für `dir` auf und löst den bisherigen ab.
    fn open(&self, dir: &Path, text: Option<String>) {
        let old = self.editor.borrow_mut().take();
        let editor = Editor::open(EditorOptions {
            project_dir: dir.to_path_buf(),
            source: Arc::clone(&self.source),
            size: self.size,
            menu: true,
        });
        if let Some(old) = &old {
            editor
                .window()
                .widget()
                .set_geometry(old.window().geometry());
            editor.set_current_tab(old.current_tab());
        }
        if let Some(text) = text {
            editor.set_source_text(&text);
        }
        let me = self.me.clone();
        editor.on_request(move |request| {
            // Erst nach dem Menü-Signal umbauen (das Fenster des alten Editors
            // ist gerade im Aufruf).
            let me = me.clone();
            ruf::Timer::single_shot(0, move || {
                if let Some(me) = me.upgrade() {
                    me.handle(request.clone());
                }
            });
        });
        editor.show();
        *self.editor.borrow_mut() = Some(editor);
        if let Some(old) = old {
            old.window().hide();
            self.retired.borrow_mut().push(old);
            let me = self.me.clone();
            ruf::Timer::single_shot(0, move || {
                if let Some(me) = me.upgrade() {
                    me.retired.borrow_mut().clear();
                }
            });
        }
    }
}
