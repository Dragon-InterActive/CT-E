//! Tab 2: Abhängigkeiten – die visuelle Schicht.
//!
//! ```text
//! ┌ Tabelle [dependencies ▾]  Plattform [          ▾] ──────────────────────────┐
//! │ Suche [serde_______________]          │ Abhängigkeiten in [dependencies]    │
//! │ ┌ Treffer ──────────────────────────┐ │ ┌─────────────────────────────────┐ │
//! │ │ serde (1.0.229)                   │ │ │ anyhow 1.0                      │ │
//! │ │ serde_json (1.0.154)              │ │ │ serde 1.0 [derive]              │ │
//! │ └───────────────────────────────────┘ │ └─────────────────────────────────┘ │
//! │ serde – A generic serialization …     │                                     │
//! │ Version [1.0.229 ▾]  [ ] optional     │                                     │
//! │ ┌ Features ─────────────────────────┐ │                                     │
//! │ │ [x] std (default)  [ ] derive …   │ │                                     │
//! │ └───────────────────────────────────┘ │                                     │
//! │ [Hinzufügen]  [Entfernen]             │                                     │
//! └───────────────────────────────────────┴─────────────────────────────────────┘
//! ```
//!
//! Gesucht wird erst ab [`MIN_QUERY_LEN`](crate::crates_io::MIN_QUERY_LEN)
//! Zeichen und erst nach einer kurzen Tipp-Pause. Netzwerkzugriffe laufen in
//! Hintergrund-Threads; veraltete Antworten werden über Generationszähler
//! verworfen.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use rncl::prelude::*;
use ruf::{GuiSender, Timer};

use super::{GAP, MARGIN, Origin, ROW, Shared, State, StatusKind, weak0, weak1};
use crate::crates_io::{CrateDetails, CrateSummary, SourceError, is_searchable};
use crate::features::{FeatureState, apply_selection, feature_states};
use crate::manifest::{DepKind, DepTable, Dependency};
use crate::tr;

/// Tipp-Pause vor der Suche.
pub const SEARCH_DELAY_MS: i32 = 400;
const FEATURE_ROW: i32 = 24;

/// Antwort der Suche aus dem Hintergrund-Thread.
pub struct SearchReply {
    generation: u64,
    result: Result<Vec<CrateSummary>, SourceError>,
}

/// Antwort auf eine Detailanfrage.
pub struct DetailsReply {
    generation: u64,
    name: String,
    result: Result<CrateDetails, SourceError>,
}

/// Das aktuell bearbeitete Crate.
#[derive(Debug, Clone)]
pub struct Selection {
    /// Name auf crates.io.
    pub crate_name: String,
    /// Schlüssel im Manifest (bei Umbenennung abweichend).
    pub key: String,
    /// Die Abhängigkeit, falls sie schon im Manifest steht.
    pub existing: Option<Dependency>,
    pub details: Option<Arc<CrateDetails>>,
}

#[derive(Default)]
pub struct DepsData {
    search_generation: u64,
    details_generation: u64,
    pub results: Vec<CrateSummary>,
    pub listed: Vec<Dependency>,
    pub selection: Option<Selection>,
    cache: HashMap<String, Arc<CrateDetails>>,
    pub states: Vec<FeatureState>,
    pub checkboxes: Vec<RnclCheckBox>,
}

pub struct DepsTab {
    pub page: RnclPanel,
    pub kind_label: RnclLabel,
    pub kind: RnclComboBox,
    pub target_label: RnclLabel,
    pub target: RnclComboBox,
    pub search_label: RnclLabel,
    pub search: RnclTextField,
    pub results: RnclListBox,
    pub results_info: RnclLabel,
    pub info: RnclLabel,
    pub version_label: RnclLabel,
    pub version: RnclComboBox,
    pub optional: RnclCheckBox,
    pub features_label: RnclLabel,
    pub features: RnclScrollView,
    pub apply: RnclButton,
    pub remove: RnclButton,
    pub list_label: RnclLabel,
    pub list: RnclListBox,
    pub debounce: Timer,
    pub data: RefCell<DepsData>,
    search_sender: OnceCell<GuiSender<SearchReply>>,
    details_sender: OnceCell<GuiSender<DetailsReply>>,
}

impl DepsTab {
    pub fn build(page: &RnclPanel) -> Self {
        let kind_label = RnclLabel::new(page, &tr!("deps.table"));
        let kind = RnclComboBox::new(page);
        kind.set_items(&DepKind::ALL.map(|k| k.table_name()));
        let target_label = RnclLabel::new(page, &tr!("deps.target"));
        let target = RnclComboBox::new(page);
        target.set_editable(true);
        target.set_tool_tip(&tr!("deps.target_tip"));

        let search_label = RnclLabel::new(page, &tr!("deps.search"));
        let search = RnclTextField::new(page);
        search.set_placeholder(&tr!("deps.search_placeholder"));
        let results = RnclListBox::new(page);
        let results_info = RnclLabel::new(page, &tr!("deps.min_chars"));
        let info = RnclLabel::new(page, &tr!("deps.nothing_selected"));
        info.set_word_wrap(true);
        let version_label = RnclLabel::new(page, &tr!("deps.version"));
        let version = RnclComboBox::new(page);
        version.set_editable(true);
        version.set_tool_tip(&tr!("deps.version_tip"));
        let optional = RnclCheckBox::new(page, &tr!("deps.optional"));
        optional.set_tool_tip(&tr!("deps.optional_tip"));
        let features_label = RnclLabel::new(page, &tr!("deps.features"));
        let features = RnclScrollView::new(page);
        let apply = RnclButton::new(page, &tr!("deps.add"));
        let remove = RnclButton::new(page, &tr!("deps.remove"));
        let list_label = RnclLabel::new(page, "");
        let list = RnclListBox::new(page);

        let debounce = Timer::new();
        debounce.set_single_shot(true);

        let tab = Self {
            page: page.clone(),
            kind_label,
            kind,
            target_label,
            target,
            search_label,
            search,
            results,
            results_info,
            info,
            version_label,
            version,
            optional,
            features_label,
            features,
            apply,
            remove,
            list_label,
            list,
            debounce,
            data: RefCell::new(DepsData::default()),
            search_sender: OnceCell::new(),
            details_sender: OnceCell::new(),
        };
        tab.set_details_enabled(false);
        tab
    }

    pub fn layout(&self, w: i32, h: i32) {
        let label_w = 90;
        // Kopfzeile: Tabelle und Plattform.
        let mut y = MARGIN;
        self.kind_label.set_geometry(MARGIN, y, label_w, ROW);
        self.kind.set_geometry(MARGIN + label_w, y, 200, ROW);
        let tx = MARGIN + label_w + 200 + 2 * GAP;
        self.target_label.set_geometry(tx, y, label_w, ROW);
        self.target.set_geometry(
            tx + label_w,
            y,
            (w - tx - label_w - MARGIN).clamp(120, 320),
            ROW,
        );
        y += ROW + 2 * GAP;

        // Zwei Spalten: links Suche + Details, rechts die vorhandenen Abhängigkeiten.
        let left_w = ((w - 3 * MARGIN) * 58 / 100).max(260);
        let right_x = 2 * MARGIN + left_w;
        let right_w = (w - right_x - MARGIN).max(150);
        let bottom = h - MARGIN;

        self.list_label.set_geometry(right_x, y, right_w, ROW);
        self.list
            .set_geometry(right_x, y + ROW, right_w, bottom - y - ROW);

        self.search_label.set_geometry(MARGIN, y, label_w, ROW);
        self.search
            .set_geometry(MARGIN + label_w, y, left_w - label_w, ROW);
        y += ROW + GAP;
        let results_h = ((bottom - y) * 28 / 100).clamp(70, 220);
        self.results.set_geometry(MARGIN, y, left_w, results_h);
        y += results_h;
        self.results_info.set_geometry(MARGIN, y, left_w, 22);
        y += 22 + GAP;
        self.info.set_geometry(MARGIN, y, left_w, 54);
        y += 54 + GAP;
        self.version_label.set_geometry(MARGIN, y, label_w, ROW);
        self.version.set_geometry(MARGIN + label_w, y, 180, ROW);
        self.optional.set_geometry(
            MARGIN + label_w + 180 + 2 * GAP,
            y,
            left_w - label_w - 180 - 2 * GAP,
            ROW,
        );
        y += ROW + GAP;
        self.features_label.set_geometry(MARGIN, y, left_w, 22);
        y += 22;
        let buttons_y = bottom - ROW;
        self.features
            .set_geometry(MARGIN, y, left_w, (buttons_y - GAP - y).max(60));
        self.apply.set_geometry(MARGIN, buttons_y, 160, ROW);
        self.remove
            .set_geometry(MARGIN + 160 + GAP, buttons_y, 160, ROW);
    }

    /// Die gewählte Tabelle (Art + Plattform).
    pub fn current_table(&self) -> DepTable {
        let kind = DepKind::ALL
            .get(self.kind.current_index().max(0) as usize)
            .copied()
            .unwrap_or(DepKind::Normal);
        DepTable::for_target(kind, self.target.current_text())
    }

    /// Befüllt Plattformliste und Abhängigkeitsliste aus dem Manifest.
    pub fn refresh(&self, state: &State) {
        let mut targets = vec![String::new()];
        targets.extend(state.manifest.targets());
        let current = self.target.current_text();
        if self.target.items() != targets {
            self.target.set_items(&targets);
        }
        self.target.set_current_text(&current);

        let table = self.current_table();
        self.list_label
            .set_text(&tr!("deps.list", table = format!("[{table}]")));
        let deps = state.manifest.dependencies(&table);
        self.list
            .set_items(&deps.iter().map(Dependency::summary).collect::<Vec<_>>());

        let mut data = self.data.borrow_mut();
        // Die Auswahl mit dem Manifest abgleichen (z. B. nach Bearbeitung im Quelltext).
        if let Some(selection) = &mut data.selection {
            selection.existing = deps.iter().find(|d| d.name == selection.key).cloned();
            if let Some(i) = deps.iter().position(|d| d.name == selection.key) {
                self.list.set_current_index(i as i32);
            }
            let label = if selection.existing.is_some() {
                tr!("deps.update")
            } else {
                tr!("deps.add")
            };
            self.apply.set_text(&label);
            self.remove.set_enabled(selection.existing.is_some());
        }
        data.listed = deps;
    }

    pub fn clear_selection(&self) {
        let mut data = self.data.borrow_mut();
        data.selection = None;
        data.states.clear();
        for check in data.checkboxes.drain(..) {
            check.widget().delete_later();
        }
        drop(data);
        self.info.set_text(&tr!("deps.nothing_selected"));
        self.version.set_items::<&str>(&[]);
        self.version.set_current_text("");
        self.optional.set_checked(false);
        self.apply.set_text(&tr!("deps.add"));
        self.set_details_enabled(false);
    }

    fn set_details_enabled(&self, enabled: bool) {
        self.version.set_enabled(enabled);
        self.optional.set_enabled(enabled);
        self.apply.set_enabled(enabled);
        self.remove.set_enabled(
            enabled
                && self
                    .data
                    .borrow()
                    .selection
                    .as_ref()
                    .is_some_and(|s| s.existing.is_some()),
        );
    }

    /// Baut die Feature-Checkboxen neu auf.
    fn show_features(&self, states: Vec<FeatureState>) {
        let mut data = self.data.borrow_mut();
        for check in data.checkboxes.drain(..) {
            check.widget().delete_later();
        }
        let content = self.features.content();
        let width = (self.features.size().width - 30).max(200);
        for (i, state) in states.iter().enumerate() {
            let text = if state.is_default {
                format!("{} {}", state.name, tr!("deps.default_marker"))
            } else {
                state.name.clone()
            };
            let check = RnclCheckBox::new(content, &text);
            check.set_geometry(GAP, GAP + i as i32 * FEATURE_ROW, width, FEATURE_ROW);
            check.set_checked(state.enabled);
            if !state.enables.is_empty() {
                check.set_tool_tip(&tr!("deps.enables", list = state.enables.join(", ")));
            }
            data.checkboxes.push(check);
        }
        self.features
            .set_content_size(0, 2 * GAP + states.len() as i32 * FEATURE_ROW);
        self.features_label.set_text(&if states.is_empty() {
            tr!("deps.no_features")
        } else {
            tr!("deps.features_count", count = states.len())
        });
        data.states = states;
    }

    /// Feature-Liste für die gewählte Version, bisherige Häkchen bleiben.
    fn rebuild_features_for_version(&self, requirement: &str) {
        let (details, mut existing, previous) = {
            let data = self.data.borrow();
            let Some(selection) = &data.selection else {
                return;
            };
            (
                selection.details.clone(),
                selection.existing.clone(),
                data.states.clone(),
            )
        };
        // Bisherige Auswahl als "Abhängigkeit" ausdrücken, damit sie beim
        // Versionswechsel erhalten bleibt.
        if !previous.is_empty() {
            let mut dep = existing.clone().unwrap_or_else(|| Dependency::new("", ""));
            apply_selection(&mut dep, &previous);
            existing = Some(dep);
        }
        let available = details
            .as_ref()
            .and_then(|d| d.matching(requirement))
            .map(|v| v.features.clone())
            .unwrap_or_default();
        self.show_features(feature_states(&available, existing.as_ref()));
    }
}

pub fn connect(shared: &Rc<Shared>) {
    let tab = &shared.deps;
    let weak = Rc::downgrade(shared);
    let _ = tab
        .search_sender
        .set(GuiSender::new(move |reply: SearchReply| {
            if let Some(s) = weak.upgrade() {
                on_search_reply(&s, reply);
            }
        }));
    let weak = Rc::downgrade(shared);
    let _ = tab
        .details_sender
        .set(GuiSender::new(move |reply: DetailsReply| {
            if let Some(s) = weak.upgrade() {
                on_details_reply(&s, reply);
            }
        }));

    let weak = Rc::downgrade(shared);
    tab.page.widget().on_resized(move |w, h| {
        if let Some(s) = weak.upgrade() {
            s.deps.layout(w, h);
        }
    });

    tab.search.on_text_changed(weak1(shared, |s, text: String| {
        let tab = &s.deps;
        if is_searchable(&text) {
            tab.results_info.set_text(&tr!("deps.waiting"));
            tab.debounce.start(super::deps_tab::SEARCH_DELAY_MS);
        } else {
            tab.debounce.stop();
            tab.data.borrow_mut().search_generation += 1; // laufende Suche verwerfen
            tab.results.clear();
            tab.results_info.set_text(&tr!("deps.min_chars"));
        }
    }));
    tab.search.on_return_pressed(weak0(shared, |s| {
        if is_searchable(&s.deps.search.text()) {
            s.deps.debounce.stop();
            start_search(s);
        }
    }));
    tab.debounce.on_timeout(weak0(shared, start_search));

    tab.results
        .on_current_index_changed(weak1(shared, |s, index: i32| {
            let summary = s
                .deps
                .data
                .borrow()
                .results
                .get(index.max(0) as usize)
                .cloned();
            if index >= 0
                && let Some(summary) = summary
            {
                s.deps.list.set_current_index(-1);
                select_crate(s, &summary.name, &summary.name);
            }
        }));
    tab.list
        .on_current_index_changed(weak1(shared, |s, index: i32| {
            let dep = s
                .deps
                .data
                .borrow()
                .listed
                .get(index.max(0) as usize)
                .cloned();
            if index >= 0
                && let Some(dep) = dep
            {
                select_crate(s, dep.crate_name(), &dep.name);
            }
        }));

    let table_changed = |s: &Rc<Shared>| {
        let state = s.state.borrow();
        s.deps.refresh(&state);
    };
    tab.kind
        .on_current_index_changed(weak1(shared, move |s, _: i32| table_changed(s)));
    tab.target
        .on_text_changed(weak1(shared, move |s, _: String| table_changed(s)));

    tab.version
        .on_text_changed(weak1(shared, |s, text: String| {
            s.deps.rebuild_features_for_version(&text);
        }));

    tab.apply
        .on_clicked(weak0(shared, apply_selection_to_manifest));
    tab.remove.on_clicked(weak0(shared, remove_selected));
}

fn start_search(s: &Rc<Shared>) {
    let tab = &s.deps;
    let query = tab.search.text();
    if !is_searchable(&query) {
        return;
    }
    let generation = {
        let mut data = tab.data.borrow_mut();
        data.search_generation += 1;
        data.search_generation
    };
    tab.results_info
        .set_text(&tr!("deps.searching", query = query.trim()));
    let source = s.crate_source();
    let sender = tab.search_sender.get().expect("in connect gesetzt").clone();
    std::thread::spawn(move || {
        let result = source.search(&query);
        sender.send(SearchReply { generation, result });
    });
}

fn on_search_reply(s: &Rc<Shared>, reply: SearchReply) {
    let tab = &s.deps;
    if reply.generation != tab.data.borrow().search_generation {
        return; // inzwischen wurde weitergetippt
    }
    match reply.result {
        Ok(results) => {
            tab.results
                .set_items(&results.iter().map(CrateSummary::label).collect::<Vec<_>>());
            tab.results_info.set_text(&if results.is_empty() {
                tr!("deps.no_results")
            } else {
                tr!("deps.results", count = results.len())
            });
            tab.data.borrow_mut().results = results;
        }
        Err(error) => {
            tab.results.clear();
            tab.data.borrow_mut().results.clear();
            tab.results_info.set_text(&tr!("deps.search_failed"));
            s.set_status(StatusKind::Error, &tr!("deps.network_error", error = error));
        }
    }
}

/// Wählt ein Crate zur Bearbeitung aus und lädt (bei Bedarf) seine Details.
fn select_crate(s: &Rc<Shared>, crate_name: &str, key: &str) {
    let tab = &s.deps;
    let existing = tab
        .data
        .borrow()
        .listed
        .iter()
        .find(|d| d.name == key)
        .cloned();
    let cached = tab.data.borrow().cache.get(crate_name).cloned();
    let generation = {
        let mut data = tab.data.borrow_mut();
        data.details_generation += 1;
        data.states.clear();
        data.selection = Some(Selection {
            crate_name: crate_name.to_owned(),
            key: key.to_owned(),
            existing,
            details: None,
        });
        data.details_generation
    };
    tab.set_details_enabled(false);
    tab.info.set_text(&tr!("deps.loading", name = crate_name));

    match cached {
        Some(details) => show_details(s, Some(details)),
        None => {
            let source = s.crate_source();
            let sender = tab
                .details_sender
                .get()
                .expect("in connect gesetzt")
                .clone();
            let name = crate_name.to_owned();
            std::thread::spawn(move || {
                let result = source.details(&name);
                sender.send(DetailsReply {
                    generation,
                    name,
                    result,
                });
            });
        }
    }
}

fn on_details_reply(s: &Rc<Shared>, reply: DetailsReply) {
    let tab = &s.deps;
    if reply.generation != tab.data.borrow().details_generation {
        return;
    }
    match reply.result {
        Ok(details) => {
            let details = Arc::new(details);
            tab.data
                .borrow_mut()
                .cache
                .insert(reply.name, Arc::clone(&details));
            show_details(s, Some(details));
        }
        Err(error) => {
            // Ohne crates.io-Daten (Pfad-/Git-Abhängigkeit, offline) bleibt
            // die Bearbeitung der vorhandenen Angaben möglich.
            if !matches!(error, SourceError::NotFound(_)) {
                s.set_status(StatusKind::Error, &tr!("deps.network_error", error = error));
            }
            show_details(s, None);
        }
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn show_details(s: &Rc<Shared>, details: Option<Arc<CrateDetails>>) {
    let tab = &s.deps;
    let selection = {
        let mut data = tab.data.borrow_mut();
        let Some(selection) = &mut data.selection else {
            return;
        };
        selection.details = details.clone();
        selection.clone()
    };

    // Kopf: Name, Kurzbeschreibung, Link.
    let mut info = format!("<b>{}</b>", escape(&selection.crate_name));
    if selection.key != selection.crate_name {
        info.push_str(&format!(" → <i>{}</i>", escape(&selection.key)));
    }
    match details.as_deref() {
        Some(d) => {
            if let Some(description) = &d.description {
                info.push_str(&format!(" – {}", escape(description)));
            }
            if let Some(link) = d.repository.as_ref().or(d.homepage.as_ref()) {
                info.push_str(&format!("<br><small>{}</small>", escape(link)));
            }
        }
        None => info.push_str(&format!(
            " – <i>{}</i>",
            escape(&tr!("deps.no_registry_data"))
        )),
    }
    tab.info.set_text(&info);

    // Versionen: neueste zuerst, zurückgezogene ausgeblendet.
    let versions: Vec<String> = details
        .as_deref()
        .map(|d| {
            d.versions
                .iter()
                .filter(|v| !v.yanked)
                .map(|v| v.num.clone())
                .collect()
        })
        .unwrap_or_default();
    let requirement = match &selection.existing {
        Some(dep) => dep.version.clone().unwrap_or_default(),
        None => details
            .as_deref()
            .and_then(|d| d.recommended())
            .map(|v| v.num.clone())
            .unwrap_or_default(),
    };

    let was = s.refreshing.replace(true);
    tab.version.set_items(&versions);
    tab.version.set_current_text(&requirement);
    tab.optional
        .set_checked(selection.existing.as_ref().is_some_and(|d| d.optional));
    s.refreshing.set(was);

    let available = details
        .as_deref()
        .and_then(|d| d.matching(&requirement))
        .map(|v| v.features.clone())
        .unwrap_or_default();
    tab.show_features(feature_states(&available, selection.existing.as_ref()));

    tab.apply.set_text(&if selection.existing.is_some() {
        tr!("deps.update")
    } else {
        tr!("deps.add")
    });
    tab.set_details_enabled(true);
    // Workspace-Abhängigkeiten erben die Version.
    if selection.existing.as_ref().is_some_and(|d| d.workspace) {
        tab.version.set_enabled(false);
    }
}

/// "Hinzufügen" bzw. "Übernehmen": schreibt Version, Features und optional ins Manifest.
fn apply_selection_to_manifest(s: &Rc<Shared>) {
    let tab = &s.deps;
    let (selection, mut states) = {
        let data = tab.data.borrow();
        let Some(selection) = data.selection.clone() else {
            return;
        };
        (selection, data.states.clone())
    };
    // Häkchen aus den Checkboxen übernehmen.
    for (state, check) in states.iter_mut().zip(tab.data.borrow().checkboxes.iter()) {
        state.enabled = check.is_checked();
    }

    let version = tab.version.current_text().trim().to_owned();
    let mut dep = selection
        .existing
        .clone()
        .unwrap_or_else(|| Dependency::new(selection.key.clone(), version.clone()));
    if !dep.workspace {
        if version.is_empty() && dep.path.is_none() && dep.git.is_none() {
            s.set_status(StatusKind::Error, &tr!("deps.version_missing"));
            return;
        }
        dep.version = (!version.is_empty()).then_some(version);
    }
    dep.optional = tab.optional.is_checked();
    apply_selection(&mut dep, &states);

    let table = tab.current_table();
    let added = selection.existing.is_none();
    s.modify(Origin::Dependencies, |m| m.set_dependency(&table, &dep));
    s.quietly(|state| tab.refresh(state));
    tab.data.borrow_mut().states = states;
    tab.set_details_enabled(true);
    let key = if added { "deps.added" } else { "deps.updated" };
    s.set_status(
        StatusKind::Success,
        &tr!(key, name = dep.name, table = table),
    );
}

fn remove_selected(s: &Rc<Shared>) {
    let tab = &s.deps;
    let Some(selection) = tab.data.borrow().selection.clone() else {
        return;
    };
    if selection.existing.is_none() {
        return;
    }
    let table = tab.current_table();
    s.modify(Origin::Dependencies, |m| {
        m.remove_dependency(&table, &selection.key);
    });
    tab.clear_selection();
    s.quietly(|state| tab.refresh(state));
    s.set_status(
        StatusKind::Success,
        &tr!("deps.removed", name = selection.key, table = table),
    );
}
