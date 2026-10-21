//! Feature-Auswahl einer Abhängigkeit: Checkbox-Zustände ⇄ Manifest.
//!
//! Regeln (wie Cargo sie auswertet):
//! * Ohne Angaben sind die `default`-Features des Crates aktiv – deshalb
//!   sind beim Hinzufügen genau diese vorausgewählt.
//! * Abgewählte Default-Features gehen nur mit `default-features = false`;
//!   dann werden alle gewünschten Features explizit aufgezählt.
//! * Sonst stehen in `features = [...]` nur die *zusätzlichen* Features.

use std::collections::BTreeMap;

use crate::manifest::Dependency;

/// Eine Zeile der Feature-Liste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureState {
    pub name: String,
    /// Was dieses Feature selbst aktiviert (für den Tooltip).
    pub enables: Vec<String>,
    /// Teil von `default`?
    pub is_default: bool,
    pub enabled: bool,
}

/// Baut die Feature-Liste für die Anzeige.
///
/// `available` sind die Features der gewählten Version (inkl. `default`),
/// `dep` die vorhandene Abhängigkeit im Manifest (oder `None` beim
/// Hinzufügen). Features aus dem Manifest, die die Version nicht kennt,
/// erscheinen trotzdem – sonst gingen sie beim Speichern verloren.
pub fn feature_states(
    available: &BTreeMap<String, Vec<String>>,
    dep: Option<&Dependency>,
) -> Vec<FeatureState> {
    let defaults: Vec<&String> = available
        .get("default")
        .map(|d| d.iter().filter(|f| available.contains_key(*f)).collect())
        .unwrap_or_default();
    let is_enabled = |name: &str, is_default: bool| match dep {
        None => is_default,
        Some(dep) => dep.features.iter().any(|f| f == name) || (dep.default_features && is_default),
    };

    let mut states: Vec<FeatureState> = available
        .iter()
        .filter(|(name, _)| name.as_str() != "default")
        .map(|(name, enables)| {
            let is_default = defaults.contains(&name);
            FeatureState {
                name: name.clone(),
                enables: enables.clone(),
                is_default,
                enabled: is_enabled(name, is_default),
            }
        })
        .collect();

    if let Some(dep) = dep {
        for extra in dep.features.iter().filter(|f| !available.contains_key(*f)) {
            states.push(FeatureState {
                name: extra.clone(),
                enables: Vec::new(),
                is_default: false,
                enabled: true,
            });
        }
    }
    states
}

/// Überträgt die Auswahl in die Abhängigkeit (`features`, `default-features`).
pub fn apply_selection(dep: &mut Dependency, states: &[FeatureState]) {
    let selected = |s: &&FeatureState| s.enabled;
    let has_defaults = states.iter().any(|s| s.is_default);
    let all_defaults_on = states.iter().filter(|s| s.is_default).all(|s| s.enabled);

    let mut features: Vec<String> = if !has_defaults || all_defaults_on {
        // Kennt das Crate keine Defaults, bleibt die bisherige Einstellung.
        if has_defaults {
            dep.default_features = true;
        }
        states
            .iter()
            .filter(selected)
            .filter(|s| !(dep.default_features && s.is_default))
            .map(|s| s.name.clone())
            .collect()
    } else {
        dep.default_features = false;
        states
            .iter()
            .filter(selected)
            .map(|s| s.name.clone())
            .collect()
    };
    // Bisherige Reihenfolge im Manifest beibehalten, Neues hinten anfügen –
    // so bleiben Änderungen in der Datei (und im Diff) minimal.
    let previous = std::mem::take(&mut dep.features);
    features.sort_by_key(|f| previous.iter().position(|p| p == f).unwrap_or(usize::MAX));
    dep.features = features;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available() -> BTreeMap<String, Vec<String>> {
        [
            ("default", vec!["std", "derive"]),
            ("std", vec![]),
            ("derive", vec!["dep:derive_impl"]),
            ("rc", vec![]),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.into_iter().map(str::to_owned).collect()))
        .collect()
    }

    fn names(states: &[FeatureState]) -> Vec<(&str, bool)> {
        states
            .iter()
            .map(|s| (s.name.as_str(), s.enabled))
            .collect()
    }

    #[test]
    fn new_dependency_uses_crate_defaults() {
        let states = feature_states(&available(), None);
        assert_eq!(
            names(&states),
            [("derive", true), ("rc", false), ("std", true)]
        );
        let mut dep = Dependency::new("x", "1");
        apply_selection(&mut dep, &states);
        assert!(dep.default_features);
        assert!(dep.features.is_empty());
    }

    #[test]
    fn extra_feature_is_listed_alone() {
        let mut states = feature_states(&available(), None);
        states[1].enabled = true; // rc
        let mut dep = Dependency::new("x", "1");
        apply_selection(&mut dep, &states);
        assert!(dep.default_features);
        assert_eq!(dep.features, ["rc"]);
    }

    #[test]
    fn deselected_default_disables_defaults() {
        let mut states = feature_states(&available(), None);
        states[2].enabled = false; // std
        let mut dep = Dependency::new("x", "1");
        apply_selection(&mut dep, &states);
        assert!(!dep.default_features);
        assert_eq!(dep.features, ["derive"]);

        // Rückweg: aus dem Manifest gelesen ergibt sich wieder dieselbe Auswahl.
        assert_eq!(feature_states(&available(), Some(&dep)), states);
    }

    #[test]
    fn unknown_manifest_features_survive() {
        let mut dep = Dependency::new("x", "1");
        dep.features = vec!["legacy".into()];
        let states = feature_states(&available(), Some(&dep));
        assert!(states.iter().any(|s| s.name == "legacy" && s.enabled));
        let mut out = dep.clone();
        apply_selection(&mut out, &states);
        assert_eq!(out.features, ["legacy"]);
    }

    #[test]
    fn manifest_order_is_kept() {
        let mut dep = Dependency::new("x", "1");
        dep.default_features = false;
        dep.features = vec!["rc".into()];
        let mut states = feature_states(&available(), Some(&dep));
        states
            .iter_mut()
            .find(|s| s.name == "derive")
            .unwrap()
            .enabled = true;
        apply_selection(&mut dep, &states);
        // Alphabetisch wäre derive vor rc – die Manifest-Reihenfolge gewinnt.
        assert_eq!(dep.features, ["rc", "derive"]);
    }

    #[test]
    fn crate_without_defaults_keeps_setting() {
        let available: BTreeMap<String, Vec<String>> = [("a".to_owned(), vec![])].into();
        let mut dep = Dependency::new("x", "1");
        dep.default_features = false;
        let mut states = feature_states(&available, Some(&dep));
        assert_eq!(names(&states), [("a", false)]);
        states[0].enabled = true;
        apply_selection(&mut dep, &states);
        assert!(!dep.default_features);
        assert_eq!(dep.features, ["a"]);
    }
}
