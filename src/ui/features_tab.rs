//! Tab 4: die eigenen Features des Pakets (`[features]`).

use std::rc::Rc;

use rncl::prelude::*;

use super::{GAP, MARGIN, Origin, ROW, Shared, State, StatusKind, weak0, weak1};
use crate::manifest::split_list;
use crate::tr;

pub struct FeaturesTab {
    pub page: RnclPanel,
    pub list_label: RnclLabel,
    pub list: RnclListBox,
    pub name_label: RnclLabel,
    pub name: RnclTextField,
    pub entries_label: RnclLabel,
    pub entries: RnclTextField,
    pub hint: RnclLabel,
    pub new: RnclButton,
    pub apply: RnclButton,
    pub remove: RnclButton,
    /// Die Feature-Namen in Listenreihenfolge.
    pub names: std::cell::RefCell<Vec<String>>,
}

impl FeaturesTab {
    pub fn build(page: &RnclPanel) -> Self {
        let list_label = RnclLabel::new(page, &tr!("features.list"));
        let list = RnclListBox::new(page);
        let name_label = RnclLabel::new(page, &tr!("features.name"));
        let name = RnclTextField::new(page);
        let entries_label = RnclLabel::new(page, &tr!("features.entries"));
        let entries = RnclTextField::new(page);
        entries.set_placeholder(&tr!("features.entries_placeholder"));
        let hint = RnclLabel::new(page, &tr!("features.hint"));
        hint.set_word_wrap(true);
        let new = RnclButton::new(page, &tr!("features.new"));
        let apply = RnclButton::new(page, &tr!("features.apply"));
        let remove = RnclButton::new(page, &tr!("features.remove"));
        Self {
            page: page.clone(),
            list_label,
            list,
            name_label,
            name,
            entries_label,
            entries,
            hint,
            new,
            apply,
            remove,
            names: Default::default(),
        }
    }

    pub fn layout(&self, w: i32, h: i32) {
        let list_w = ((w - 3 * MARGIN) * 40 / 100).max(180);
        self.list_label.set_geometry(MARGIN, MARGIN, list_w, ROW);
        self.list
            .set_geometry(MARGIN, MARGIN + ROW, list_w, h - 2 * MARGIN - ROW);

        let x = 2 * MARGIN + list_w;
        let form_w = w - x - MARGIN;
        let label_w = 110;
        let mut y = MARGIN + ROW;
        self.name_label.set_geometry(x, y, label_w, ROW);
        self.name
            .set_geometry(x + label_w, y, form_w - label_w, ROW);
        y += ROW + GAP;
        self.entries_label.set_geometry(x, y, label_w, ROW);
        self.entries
            .set_geometry(x + label_w, y, form_w - label_w, ROW);
        y += ROW + 2 * GAP;
        let button_w = 130;
        self.new.set_geometry(x + label_w, y, button_w, ROW);
        self.apply
            .set_geometry(x + label_w + button_w + GAP, y, button_w, ROW);
        self.remove
            .set_geometry(x + label_w + 2 * (button_w + GAP), y, button_w, ROW);
        y += ROW + 3 * GAP;
        self.hint
            .set_geometry(x, y, form_w, (h - y - MARGIN).clamp(40, 200));
    }

    pub fn refresh(&self, state: &State) {
        let features = state.manifest.features();
        let selected = self.name.text();
        self.list.set_items(
            &features
                .iter()
                .map(|(name, entries)| format!("{name} = [{}]", entries.join(", ")))
                .collect::<Vec<_>>(),
        );
        let names: Vec<String> = features.iter().map(|(n, _)| n.clone()).collect();
        if let Some(i) = names.iter().position(|n| *n == selected) {
            self.list.set_current_index(i as i32);
        }
        *self.names.borrow_mut() = names;
    }
}

pub fn connect(shared: &Rc<Shared>) {
    let tab = &shared.features;
    let weak = Rc::downgrade(shared);
    tab.page.widget().on_resized(move |w, h| {
        if let Some(s) = weak.upgrade() {
            s.features.layout(w, h);
        }
    });

    tab.list
        .on_current_index_changed(weak1(shared, |s, index: i32| {
            let tab = &s.features;
            let Some(name) = tab.names.borrow().get(index.max(0) as usize).cloned() else {
                return;
            };
            if index < 0 {
                return;
            }
            let entries = s
                .state
                .borrow()
                .manifest
                .features()
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, e)| e)
                .unwrap_or_default();
            tab.name.set_text(&name);
            tab.entries.set_text(&entries.join(", "));
        }));

    tab.new.on_clicked(weak0(shared, |s| {
        let tab = &s.features;
        tab.list.set_current_index(-1);
        tab.name.set_text("");
        tab.entries.set_text("");
    }));

    tab.apply.on_clicked(weak0(shared, |s| {
        let tab = &s.features;
        let name = tab.name.text().trim().to_owned();
        if name.is_empty() {
            s.set_status(StatusKind::Error, &tr!("features.name_missing"));
            return;
        }
        let entries = split_list(&tab.entries.text());
        // Umbenennen: der in der Liste markierte Name weicht vom Eingabefeld ab.
        let old = tab
            .names
            .borrow()
            .get(tab.list.current_index().max(0) as usize)
            .cloned();
        let old = old.filter(|o| tab.list.current_index() >= 0 && *o != name);
        s.modify(Origin::Features, |m| {
            if let Some(old) = &old {
                m.remove_feature(old);
            }
            m.set_feature(&name, &entries);
        });
        s.quietly(|state| tab.refresh(state));
        s.set_status(StatusKind::Success, &tr!("features.saved", name = name));
    }));

    tab.remove.on_clicked(weak0(shared, |s| {
        let tab = &s.features;
        let name = tab.name.text().trim().to_owned();
        let mut removed = false;
        s.modify(Origin::Features, |m| removed = m.remove_feature(&name));
        if removed {
            tab.name.set_text("");
            tab.entries.set_text("");
            s.quietly(|state| tab.refresh(state));
            s.set_status(StatusKind::Success, &tr!("features.removed", name = name));
        }
    }));
}
