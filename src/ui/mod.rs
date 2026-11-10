//! Die Oberfläche des CT-Editors (RUF/RNCL).
//!
//! # Aufbau
//!
//! Ein Fenster mit Werkzeugleiste, fünf Registerkarten und Statuszeile:
//!
//! | Tab | Modul | Inhalt |
//! |---|---|---|
//! | 1 Quelltext | [`source_tab`] | die `Cargo.toml` als Text, Syntaxprüfung beim Tippen |
//! | 2 Abhängigkeiten | [`deps_tab`] | die visuelle Schicht: Suche auf crates.io, Features per Checkbox |
//! | 3 Paket | [`package_tab`] | Formular für `[package]` |
//! | 4 Features | [`features_tab`] | eigene `[features]` |
//! | 5 Erweitert | [`advanced_tab`] | jeder weitere Abschnitt als TOML (`[lib]`, `[[bin]]`, `[profile.*]`, `[workspace]` …) |
//!
//! # Datenfluss
//!
//! Es gibt genau ein [`Manifest`] (in [`Shared::state`]). Visuelle Tabs ändern
//! es über [`Shared::modify`]; danach wird der Quelltext neu erzeugt und alle
//! *anderen* Tabs aktualisiert (der auslösende Tab nicht – sonst spränge
//! z. B. der Cursor im Eingabefeld). Der Quelltext-Tab parst seinen Text
//! nach kurzer Tipp-Pause; solange er einen Syntaxfehler enthält, sind die
//! visuellen Tabs gesperrt, damit nichts den Text des Anwenders überschreibt.
//!
//! Alle Widgets sind öffentlich erreichbar (`editor.shared().deps.search` …) –
//! für Integrationstests und für KI-Agenten, die den Editor fernsteuern.

pub mod advanced_tab;
pub mod deps_tab;
pub mod features_tab;
pub mod package_tab;
pub mod source_tab;
pub mod standalone;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use rncl::prelude::*;
use ruf::dialog;

use crate::cargo_cli::{self, CheckResult};
use crate::cli::Tab;
use crate::crates_io::CrateSource;
use crate::manifest::{Manifest, ParseError, sanitize_package_name};
use crate::{i18n, tr};

use self::advanced_tab::AdvancedTab;
use self::deps_tab::DepsTab;
use self::features_tab::FeaturesTab;
use self::package_tab::PackageTab;
use self::source_tab::SourceTab;

/// Abstände und Maße für das (absolute) Layout.
pub(crate) const MARGIN: i32 = 8;
pub(crate) const ROW: i32 = 28;
pub(crate) const GAP: i32 = 6;

/// Wer eine Änderung ausgelöst hat – dieser Tab wird danach nicht neu befüllt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Source,
    Dependencies,
    Package,
    Features,
    Advanced,
    /// Laden, Neu laden – alle Tabs werden befüllt.
    External,
}

/// Art einer Statusmeldung (bestimmt die Farbe).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Error,
}

/// Der veränderliche Zustand des Editors.
#[derive(Debug)]
pub struct State {
    /// Das zuletzt gültige Manifest.
    pub manifest: Manifest,
    /// Der aktuelle Quelltext (kann ungültig sein, siehe `parse_error`).
    pub text: String,
    /// Inhalt der Datei beim letzten Laden/Speichern; `None`, solange sie nicht existiert.
    pub saved_text: Option<String>,
    pub parse_error: Option<ParseError>,
}

/// Einstellungen zum Öffnen.
pub struct EditorOptions {
    pub project_dir: PathBuf,
    pub source: Arc<dyn CrateSource>,
    pub size: (i32, i32),
    /// Menü „Datei“ (Öffnen, Sprache, Beenden) für den eigenständigen
    /// Betrieb; von Freya TDE gestartet ohne Menü.
    pub menu: bool,
}

/// Was der Editor über das Menü „Datei“ von seinem Rahmen verlangt
/// ([`Editor::on_request`], im eigenständigen Betrieb [`standalone::Standalone`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Die `Cargo.toml` in diesem Projektordner öffnen.
    Open(PathBuf),
    /// Die Oberfläche in dieser Sprache neu aufbauen.
    Language(String),
    /// Diese Darstellung verwenden (und merken).
    Theme(crate::cli::Theme),
}

thread_local! {
    static THEME: std::cell::Cell<crate::cli::Theme> = const { std::cell::Cell::new(crate::cli::Theme::System) };
}

/// Stellt die Darstellung ein (System, Hell, Dunkel).
pub fn set_theme(theme: crate::cli::Theme) {
    use crate::cli::Theme;
    THEME.with(|t| t.set(theme));
    Application::set_color_scheme(match theme {
        Theme::System => ruf::ColorScheme::Unknown,
        Theme::Light => ruf::ColorScheme::Light,
        Theme::Dark => ruf::ColorScheme::Dark,
    });
}

/// Die eingestellte Darstellung.
pub fn theme() -> crate::cli::Theme {
    THEME.with(std::cell::Cell::get)
}

/// Empfänger der Anforderungen aus dem Menü.
type RequestHandler = Box<dyn Fn(Request)>;

/// Menüleiste des eigenständigen Editors.
fn menu_bar() -> MenuBar {
    let current = i18n::language();
    let mut languages = Menu::new();
    for language in i18n::available() {
        let name = i18n::name_of(&language);
        languages = if language == current {
            languages.disabled_item(&format!("lang.{language}"), &format!("{name} ✓"))
        } else {
            languages.item(&format!("lang.{language}"), &name, None)
        };
    }
    let mut themes = Menu::new();
    for t in crate::cli::Theme::ALL {
        let id = format!("theme.{}", t.as_str());
        let name = tr!(&format!("menu.theme_{}", t.as_str()));
        themes = if t == theme() {
            themes.disabled_item(&id, &format!("{name} ✓"))
        } else {
            themes.item(&id, &name, None)
        };
    }
    vec![(
        tr!("menu.file"),
        Menu::new()
            .item("file.open", &tr!("menu.open"), Some("Ctrl+O"))
            .item("file.save", &tr!("menu.save"), Some("Ctrl+S"))
            .item("file.save_as", &tr!("menu.save_as"), Some("Ctrl+Shift+S"))
            .separator()
            .submenu(&tr!("menu.language"), languages)
            .submenu(&tr!("menu.appearance"), themes)
            .separator()
            .item("file.quit", &tr!("menu.quit"), Some("Ctrl+Q")),
    )]
}

/// Alles, was die Tabs gemeinsam brauchen. Wird als `Rc` geteilt; Closures
/// halten nur `Weak`-Verweise, damit keine Referenzzyklen entstehen.
pub struct Shared {
    pub window: RnclWindow,
    pub path_label: RnclLabel,
    pub check_button: RnclButton,
    pub reload_button: RnclButton,
    pub save_button: RnclButton,
    pub tabs: RnclTabView,
    pub status: RnclLabel,

    pub source: SourceTab,
    pub deps: DepsTab,
    pub package: PackageTab,
    pub features: FeaturesTab,
    pub advanced: AdvancedTab,

    pub state: RefCell<State>,
    /// Während Tabs befüllt werden: Signale der Widgets ignorieren.
    refreshing: Cell<bool>,
    /// Zeigte die Statuszeile zuletzt einen Syntaxfehler?
    had_error: Cell<bool>,
    manifest_path: PathBuf,
    crate_source: Arc<dyn CrateSource>,
    /// Empfänger der Anforderungen aus dem Menü (eigenständiger Betrieb).
    requests: RefCell<Option<RequestHandler>>,
    /// Eigenständig mit Menü (eine neue Datei fragt beim Speichern nach dem Ort).
    standalone: bool,
}

/// Der CT-Editor.
///
/// ```no_run
/// use std::sync::Arc;
/// use ct_editor::crates_io::CratesIo;
/// use ct_editor::ui::{Editor, EditorOptions};
/// use rncl::prelude::*;
///
/// let app = Application::new();
/// let editor = Editor::open(EditorOptions {
///     project_dir: "mein-projekt".into(),
///     source: Arc::new(CratesIo::new()),
///     size: (1100, 720),
///     menu: false,
/// });
/// editor.show();
/// std::process::exit(app.exec());
/// ```
pub struct Editor {
    shared: Rc<Shared>,
}

impl Editor {
    /// Öffnet `Cargo.toml` im Projektordner bzw. bereitet eine neue vor.
    ///
    /// Eine vorhandene Datei wird geladen – auch mit Syntaxfehlern (die dann
    /// im Quelltext-Tab markiert sind). Fehlt sie, entsteht ein neues Manifest
    /// mit dem Ordnernamen als Paketname; gespeichert wird erst auf Wunsch.
    pub fn open(options: EditorOptions) -> Self {
        let manifest_path = options.project_dir.join("Cargo.toml");

        let window = RnclWindow::new("CT-Editor");
        window.set_size(options.size.0, options.size.1);
        if options.menu {
            window.set_menu_bar(menu_bar());
        }
        let path_label = RnclLabel::new(&window, "");
        // Lange Pfade in der Mitte kürzen (Dateiname bleibt sichtbar), ganz im Tooltip.
        path_label.set_elide(Elide::Middle);
        let check_button = RnclButton::new(&window, &tr!("app.check"));
        let reload_button = RnclButton::new(&window, &tr!("app.reload"));
        let save_button = RnclButton::new(&window, &tr!("app.save"));
        check_button.set_tool_tip(&tr!("app.check_tip"));
        reload_button.set_tool_tip(&tr!("app.reload_tip"));
        save_button.set_tool_tip(&tr!("app.save_tip"));
        let tabs = RnclTabView::new(&window);
        let status = RnclLabel::new(&window, "");

        let source = SourceTab::build(&tabs.add_tab(&tr!("tab.source")));
        let deps = DepsTab::build(&tabs.add_tab(&tr!("tab.dependencies")));
        let package = PackageTab::build(&tabs.add_tab(&tr!("tab.package")));
        let features = FeaturesTab::build(&tabs.add_tab(&tr!("tab.features")));
        let advanced = AdvancedTab::build(&tabs.add_tab(&tr!("tab.advanced")));

        let state = State {
            manifest: Manifest::default(),
            text: String::new(),
            saved_text: None,
            parse_error: None,
        };

        let shared = Rc::new(Shared {
            window,
            path_label,
            check_button,
            reload_button,
            save_button,
            tabs,
            status,
            source,
            deps,
            package,
            features,
            advanced,
            state: RefCell::new(state),
            refreshing: Cell::new(false),
            had_error: Cell::new(false),
            manifest_path,
            crate_source: options.source,
            requests: RefCell::new(None),
            standalone: options.menu,
        });

        Shared::connect(&shared);
        source_tab::connect(&shared);
        deps_tab::connect(&shared);
        package_tab::connect(&shared);
        features_tab::connect(&shared);
        advanced_tab::connect(&shared);

        shared.load_from_disk();
        let (w, h) = options.size;
        shared.layout(w, h);
        Self { shared }
    }

    pub fn show(&self) {
        self.shared.window.show();
    }

    pub fn window(&self) -> &RnclWindow {
        &self.shared.window
    }

    /// Zugriff auf alle Widgets und den Zustand (Tests, Automatisierung).
    pub fn shared(&self) -> &Rc<Shared> {
        &self.shared
    }

    pub fn manifest_path(&self) -> &Path {
        &self.shared.manifest_path
    }

    /// Der aktuelle Quelltext.
    pub fn text(&self) -> String {
        self.shared.state.borrow().text.clone()
    }

    pub fn is_dirty(&self) -> bool {
        self.shared.is_dirty()
    }

    pub fn parse_error(&self) -> Option<ParseError> {
        self.shared.state.borrow().parse_error.clone()
    }

    /// Ersetzt den Quelltext und wertet ihn sofort aus (wie Tippen + Pause).
    pub fn set_source_text(&self, text: &str) {
        self.shared.source.editor.set_text(text);
        self.shared.flush_source();
    }

    /// Speichert die Datei; liefert den Pfad oder eine (übersetzte) Fehlermeldung.
    pub fn save(&self) -> Result<PathBuf, String> {
        self.shared.save()
    }

    /// „Speichern unter“ ohne Dialog: schreibt `<ordner von chosen>/Cargo.toml`
    /// und lässt den Rahmen diese Datei öffnen ([`Request::Open`]).
    pub fn save_as(&self, chosen: &Path) -> Result<PathBuf, String> {
        self.shared.save_as(chosen)
    }

    pub fn select_tab(&self, tab: Tab) {
        self.shared.tabs.set_current_index(tab.index());
    }

    /// Index der gezeigten Registerkarte.
    pub fn current_tab(&self) -> i32 {
        self.shared.tabs.current_index()
    }

    /// Zeigt die Registerkarte mit Index `index` (siehe [`Editor::current_tab`]).
    pub fn set_current_tab(&self, index: i32) {
        self.shared.tabs.set_current_index(index);
    }

    /// Wer „Öffnen“ und „Sprache“ aus dem Menü ausführt. Nachfragen zu
    /// ungespeicherten Änderungen stellt der Editor vorher selbst.
    pub fn on_request(&self, f: impl Fn(Request) + 'static) {
        *self.shared.requests.borrow_mut() = Some(Box::new(f));
    }
}

/// Erzeugt eine Closure, die nur läuft, solange der Editor lebt.
pub(crate) fn weak0(
    shared: &Rc<Shared>,
    f: impl Fn(&Rc<Shared>) + 'static,
) -> impl FnMut() + 'static {
    let weak: Weak<Shared> = Rc::downgrade(shared);
    move || {
        if let Some(shared) = weak.upgrade() {
            f(&shared);
        }
    }
}

/// Wie [`weak0`], mit einem Argument. Ignoriert Signale während des Befüllens.
pub(crate) fn weak1<A>(
    shared: &Rc<Shared>,
    f: impl Fn(&Rc<Shared>, A) + 'static,
) -> impl FnMut(A) + 'static {
    let weak: Weak<Shared> = Rc::downgrade(shared);
    move |a| {
        if let Some(shared) = weak.upgrade()
            && !shared.is_refreshing()
        {
            f(&shared, a);
        }
    }
}

impl Shared {
    fn connect(shared: &Rc<Shared>) {
        shared
            .save_button
            .on_clicked(weak0(shared, |s| s.save_interactive()));
        shared
            .reload_button
            .on_clicked(weak0(shared, |s| s.reload_interactive()));
        shared
            .check_button
            .on_clicked(weak0(shared, |s| s.check_interactive()));
        // Mit Menü trägt der Eintrag „Speichern“ das Kürzel.
        if !shared.standalone {
            shared
                .window
                .add_shortcut("Ctrl+S", weak0(shared, |s| s.save_interactive()));
        }
        shared
            .window
            .add_shortcut("F5", weak0(shared, |s| s.check_interactive()));

        let weak = Rc::downgrade(shared);
        shared.window.on_resized(move |w, h| {
            if let Some(s) = weak.upgrade() {
                s.layout(w, h);
            }
        });

        let weak = Rc::downgrade(shared);
        shared.window.on_close_requested(move || {
            let Some(s) = weak.upgrade() else { return true };
            s.flush_source();
            !s.is_dirty()
                || dialog::confirm(
                    Some(s.window.widget()),
                    &tr!("dialog.unsaved_title"),
                    &tr!("dialog.unsaved_close"),
                )
        });

        // Menü „Datei“ (nur eigenständig).
        shared
            .window
            .on_menu(weak1(shared, |s, id: String| s.menu(&id)));

        // Beim Verlassen des Quelltexts sofort auswerten, nicht erst nach der Tipp-Pause.
        shared
            .tabs
            .on_current_index_changed(weak1(shared, |s, _| s.flush_source()));
    }

    /// Ein Eintrag des Menüs „Datei“ wurde gewählt.
    fn menu(&self, id: &str) {
        match id {
            "file.open" => {
                self.flush_source();
                if self.is_dirty()
                    && !dialog::confirm(
                        Some(self.window.widget()),
                        &tr!("dialog.unsaved_title"),
                        &tr!("dialog.unsaved_open"),
                    )
                {
                    return;
                }
                let filters = [dialog::FileFilter::new(
                    &tr!("dialog.open_filter"),
                    &["Cargo.toml"],
                )];
                if let Some(dir) = dialog::open_file(
                    Some(self.window.widget()),
                    &tr!("dialog.open_title"),
                    &filters,
                )
                .and_then(|file| file.parent().map(Path::to_path_buf))
                {
                    self.request(Request::Open(dir));
                }
            }
            "file.save" => self.save_interactive(),
            "file.save_as" => self.save_as_interactive(),
            "file.quit" => self.window.widget().close(),
            other => {
                if let Some(language) = other.strip_prefix("lang.") {
                    self.flush_source();
                    self.request(Request::Language(language.to_owned()));
                } else if let Some(theme) = other
                    .strip_prefix("theme.")
                    .and_then(crate::cli::Theme::parse)
                {
                    self.flush_source();
                    self.request(Request::Theme(theme));
                }
            }
        }
    }

    fn request(&self, request: Request) {
        if let Some(f) = self.requests.borrow().as_ref() {
            f(request);
        }
    }

    fn layout(&self, width: i32, height: i32) {
        let buttons = [&self.save_button, &self.reload_button, &self.check_button];
        // Gleich breite Knöpfe, mindestens so breit wie der längste Text in
        // der Schrift des jeweiligen Systems.
        let button_w = buttons
            .iter()
            .map(|b| {
                b.widget().adjust_size();
                b.size().width + 16
            })
            .max()
            .unwrap_or(0)
            .max(130);
        let mut x = width - MARGIN - button_w;
        for button in buttons {
            button.set_geometry(x, MARGIN, button_w, ROW);
            x -= button_w + GAP;
        }
        self.path_label
            .set_geometry(MARGIN, MARGIN, (x + button_w - MARGIN).max(50), ROW);
        let status_h = 24;
        let top = MARGIN + ROW + GAP;
        self.tabs.set_geometry(
            MARGIN,
            top,
            width - 2 * MARGIN,
            height - top - status_h - MARGIN - GAP,
        );
        self.status.set_geometry(
            MARGIN,
            height - MARGIN - status_h,
            width - 2 * MARGIN,
            status_h,
        );
    }

    /// Führt `f` mit dem Zustand aus, ohne dass Widget-Signale Änderungen auslösen
    /// (zum Befüllen eines Tabs nach einer eigenen Änderung).
    pub fn quietly(&self, f: impl FnOnce(&State)) {
        let was = self.refreshing.replace(true);
        {
            let state = self.state.borrow();
            f(&state);
        }
        self.refreshing.set(was);
    }

    pub fn is_refreshing(&self) -> bool {
        self.refreshing.get()
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn crate_source(&self) -> Arc<dyn CrateSource> {
        Arc::clone(&self.crate_source)
    }

    pub fn is_dirty(&self) -> bool {
        let state = self.state.borrow();
        state.saved_text.as_deref() != Some(state.text.as_str())
    }

    pub fn set_status(&self, kind: StatusKind, text: &str) {
        self.status.set_text(text);
        self.status.set_style_sheet(match kind {
            StatusKind::Info => "",
            StatusKind::Success => "color: #1e8449;",
            StatusKind::Error => "color: #c0392b; font-weight: bold;",
        });
    }

    /// Ändert das Manifest aus einem visuellen Tab heraus.
    ///
    /// Tut nichts, solange der Quelltext einen Syntaxfehler hat (die visuellen
    /// Tabs sind dann ohnehin gesperrt).
    pub fn modify(&self, origin: Origin, f: impl FnOnce(&mut Manifest)) {
        {
            let mut state = self.state.borrow_mut();
            if state.parse_error.is_some() {
                return;
            }
            f(&mut state.manifest);
            state.text = state.manifest.to_text();
        }
        self.refresh(origin);
    }

    /// Wie [`Shared::modify`], aber die Änderung darf fehlschlagen (Tab "Erweitert").
    pub fn try_modify(
        &self,
        origin: Origin,
        f: impl FnOnce(&mut Manifest) -> Result<(), ParseError>,
    ) -> Result<(), ParseError> {
        {
            let mut state = self.state.borrow_mut();
            if let Some(error) = &state.parse_error {
                return Err(error.clone());
            }
            let mut copy = state.manifest.clone();
            f(&mut copy)?;
            state.text = copy.to_text();
            state.manifest = copy;
        }
        self.refresh(origin);
        Ok(())
    }

    /// Übernimmt einen neuen Quelltext (aus dem Quelltext-Tab oder vom Laden).
    pub fn apply_source_text(&self, text: &str, origin: Origin) {
        {
            let mut state = self.state.borrow_mut();
            state.text = text.to_owned();
            match Manifest::parse(text) {
                Ok(manifest) => {
                    state.manifest = manifest;
                    state.parse_error = None;
                }
                Err(error) => state.parse_error = Some(error),
            }
        }
        self.refresh(origin);
    }

    /// Wertet einen noch ausstehenden Quelltext sofort aus.
    pub fn flush_source(&self) {
        if self.source.debounce.is_active() {
            self.source.debounce.stop();
            let text = self.source.editor.text();
            self.apply_source_text(&text, Origin::Source);
        }
    }

    /// Befüllt alle Tabs außer `origin` aus dem Zustand.
    pub fn refresh(&self, origin: Origin) {
        let was = self.refreshing.replace(true);
        {
            let state = self.state.borrow();
            if origin != Origin::Source {
                self.source.refresh(&state);
            }
            let valid = state.parse_error.is_none();
            for (page, tab_origin) in [
                (self.deps.page.widget(), Origin::Dependencies),
                (self.package.page.widget(), Origin::Package),
                (self.features.page.widget(), Origin::Features),
                (self.advanced.page.widget(), Origin::Advanced),
            ] {
                page.set_enabled(valid);
                if valid && origin != tab_origin {
                    match tab_origin {
                        Origin::Dependencies => self.deps.refresh(&state),
                        Origin::Package => self.package.refresh(&state),
                        Origin::Features => self.features.refresh(&state),
                        Origin::Advanced => self.advanced.refresh(&state),
                        _ => {}
                    }
                }
            }
            self.source.show_error(state.parse_error.as_ref());
        }
        self.refreshing.set(was);
        self.update_title();
        self.update_status();
    }

    fn update_title(&self) {
        let dirty = if self.is_dirty() { " *" } else { "" };
        self.window.set_title(&format!(
            "CT-Editor – {}{dirty}",
            self.manifest_path.display()
        ));
        let state = self.state.borrow();
        let path = self.manifest_path.display().to_string();
        let text = if state.saved_text.is_none() {
            tr!("app.path_new", path = path)
        } else {
            tr!("app.path", path = path)
        };
        self.path_label.set_text(&text);
        self.path_label.set_tool_tip(&text);
    }

    fn update_status(&self) {
        let state = self.state.borrow();
        if let Some(error) = &state.parse_error {
            self.set_status(
                StatusKind::Error,
                &tr!(
                    "status.parse_error",
                    line = error.line + 1,
                    message = error.message
                ),
            );
            self.had_error.set(true);
        } else if self.had_error.replace(false) {
            self.set_status(StatusKind::Success, &tr!("status.valid"));
        }
    }

    fn load_from_disk(&self) {
        let text = std::fs::read_to_string(&self.manifest_path);
        match text {
            Ok(text) => {
                self.state.borrow_mut().saved_text = Some(text.clone());
                self.apply_source_text(&text, Origin::External);
                self.set_status(
                    StatusKind::Info,
                    &tr!("status.loaded", path = self.manifest_path.display()),
                );
            }
            Err(_) => {
                let name = self
                    .manifest_path
                    .parent()
                    .and_then(|p| std::path::absolute(p).ok())
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| "neues-projekt".to_owned());
                let manifest = Manifest::new_package(&sanitize_package_name(&name));
                self.state.borrow_mut().saved_text = None;
                self.apply_source_text(&manifest.to_text(), Origin::External);
                self.set_status(StatusKind::Info, &tr!("status.created"));
            }
        }
        self.deps.clear_selection();
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let path = self.manifest_path.clone();
        self.save_to(&path)
    }

    /// Speichert den Text nach `path` (Ordner werden angelegt). Ist `path`
    /// nicht die geöffnete Datei, bleibt der Editor bei ihr (siehe [`Shared::save_as`]).
    fn save_to(&self, path: &Path) -> Result<PathBuf, String> {
        self.flush_source();
        let text = {
            let state = self.state.borrow();
            if let Some(error) = &state.parse_error {
                return Err(tr!(
                    "status.parse_error",
                    line = error.line + 1,
                    message = error.message
                ));
            }
            state.text.clone()
        };
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir).map_err(|e| tr!("status.save_failed", error = e))?;
        }
        std::fs::write(path, &text).map_err(|e| tr!("status.save_failed", error = e))?;
        if path == self.manifest_path {
            self.state.borrow_mut().saved_text = Some(text);
            self.update_title();
        }
        self.set_status(
            StatusKind::Success,
            &tr!("status.saved", path = path.display()),
        );
        Ok(path.to_path_buf())
    }

    /// „Speichern unter“: schreibt `<ordner>/Cargo.toml` (Cargo liest nur
    /// diesen Namen; ein anderer Dateiname zählt als Ordner) und öffnet sie
    /// danach (eigenständiger Betrieb).
    pub fn save_as(&self, chosen: &Path) -> Result<PathBuf, String> {
        let dir = if chosen.is_dir() {
            chosen.to_path_buf()
        } else {
            chosen
                .parent()
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
        };
        let saved = self.save_to(&dir.join("Cargo.toml"))?;
        if saved != self.manifest_path {
            self.request(Request::Open(dir));
        }
        Ok(saved)
    }

    fn save_as_interactive(&self) {
        let filters = [dialog::FileFilter::new(
            &tr!("dialog.open_filter"),
            &["Cargo.toml"],
        )];
        let Some(chosen) = dialog::save_file(
            Some(self.window.widget()),
            &tr!("dialog.save_as_title"),
            "Cargo.toml",
            &filters,
        ) else {
            return;
        };
        if let Err(message) = self.save_as(&chosen) {
            self.set_status(StatusKind::Error, &message);
            dialog::error(
                Some(self.window.widget()),
                &tr!("dialog.save_title"),
                &message,
            );
        }
    }

    fn save_interactive(&self) {
        // Eigenständig und noch nie gespeichert: wohin? (wie „Speichern unter“)
        if self.standalone && self.state.borrow().saved_text.is_none() {
            self.save_as_interactive();
            return;
        }
        if let Err(message) = self.save() {
            self.set_status(StatusKind::Error, &message);
            dialog::error(
                Some(self.window.widget()),
                &tr!("dialog.save_title"),
                &message,
            );
        }
    }

    fn reload_interactive(&self) {
        self.flush_source();
        if self.is_dirty()
            && !dialog::confirm(
                Some(self.window.widget()),
                &tr!("dialog.unsaved_title"),
                &tr!("dialog.unsaved_reload"),
            )
        {
            return;
        }
        self.load_from_disk();
    }

    fn check_interactive(self: &Rc<Self>) {
        self.flush_source();
        if self.is_dirty() {
            if !dialog::confirm(
                Some(self.window.widget()),
                &tr!("dialog.check_title"),
                &tr!("dialog.check_save_first"),
            ) {
                return;
            }
            if let Err(message) = self.save() {
                self.set_status(StatusKind::Error, &message);
                return;
            }
        }
        self.check_with_cargo();
    }

    /// Lässt Cargo das gespeicherte Manifest prüfen (im Hintergrund).
    pub fn check_with_cargo(self: &Rc<Self>) {
        self.set_status(StatusKind::Info, &tr!("status.checking"));
        self.check_button.set_enabled(false);
        let weak = Rc::downgrade(self);
        let sender = ruf::GuiSender::new(move |result: CheckResult| {
            let Some(s) = weak.upgrade() else { return };
            s.check_button.set_enabled(true);
            match result {
                CheckResult::Valid { warnings } if warnings.is_empty() => {
                    s.set_status(StatusKind::Success, &tr!("status.cargo_ok"));
                }
                CheckResult::Valid { warnings } => {
                    s.set_status(
                        StatusKind::Success,
                        &tr!("status.cargo_warnings", count = warnings.len()),
                    );
                    dialog::warning(
                        Some(s.window.widget()),
                        &tr!("dialog.check_title"),
                        &warnings.join("\n"),
                    );
                }
                CheckResult::NoTargets => {
                    s.set_status(StatusKind::Info, &tr!("status.cargo_no_targets"));
                }
                CheckResult::Invalid(message) => {
                    s.set_status(StatusKind::Error, &tr!("status.cargo_invalid"));
                    dialog::error(
                        Some(s.window.widget()),
                        &tr!("dialog.check_title"),
                        &message,
                    );
                }
                CheckResult::CargoMissing(error) => {
                    s.set_status(
                        StatusKind::Error,
                        &tr!("status.cargo_missing", error = error),
                    );
                }
            }
        });
        let path = self.manifest_path.clone();
        std::thread::spawn(move || {
            sender.send(cargo_cli::check_manifest(&path));
        });
    }
}
