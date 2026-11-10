//! Zugriff auf crates.io: Suche und Crate-Details inklusive Features.
//!
//! Die Aufrufe blockieren und laufen deshalb im Editor immer in einem
//! Hintergrund-Thread (Ergebnis per `ruf::GuiSender` zurück in die Oberfläche).
//!
//! Über das Trait [`CrateSource`] lässt sich die Datenquelle austauschen –
//! z. B. gegen [`StaticSource`] in Tests oder eine eigene Registry.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use serde::Deserialize;

/// Erst ab so vielen Zeichen wird gesucht (Vorgabe Freya TDE, schont die API).
pub const MIN_QUERY_LEN: usize = 3;

/// Darf mit diesem Text gesucht werden?
pub fn is_searchable(query: &str) -> bool {
    query.trim().chars().count() >= MIN_QUERY_LEN
}

/// Ein Suchtreffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateSummary {
    pub name: String,
    /// Empfohlene Version (stabil, nicht zurückgezogen), falls bekannt.
    pub version: Option<String>,
    pub description: Option<String>,
    pub downloads: u64,
}

impl CrateSummary {
    /// Anzeige in der Trefferliste: `serde (1.0.229)`.
    pub fn label(&self) -> String {
        match &self.version {
            Some(v) => format!("{} ({v})", self.name),
            None => self.name.clone(),
        }
    }
}

/// Eine veröffentlichte Version mit ihren Features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateVersion {
    pub num: String,
    pub yanked: bool,
    pub rust_version: Option<String>,
    /// Feature-Name → aktivierte Einträge (wie `[features]` im Manifest).
    pub features: BTreeMap<String, Vec<String>>,
}

impl CrateVersion {
    /// Die Features, die `default` direkt einschaltet.
    pub fn default_features(&self) -> Vec<String> {
        self.features
            .get("default")
            .map(|d| {
                d.iter()
                    .filter(|f| self.features.contains_key(*f))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Ist das eine Vorabversion (`1.0.0-beta.1`)?
    pub fn is_prerelease(&self) -> bool {
        self.num.contains('-')
    }
}

/// Alles, was der Editor über ein Crate anzeigt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateDetails {
    pub name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    /// Empfohlene Version laut crates.io.
    pub default_version: Option<String>,
    /// Neueste zuerst.
    pub versions: Vec<CrateVersion>,
}

impl CrateDetails {
    pub fn version(&self, num: &str) -> Option<&CrateVersion> {
        self.versions.iter().find(|v| v.num == num)
    }

    /// Die vorgeschlagene Version: die von crates.io empfohlene, sonst die
    /// neueste stabile, nicht zurückgezogene.
    pub fn recommended(&self) -> Option<&CrateVersion> {
        self.default_version
            .as_deref()
            .and_then(|n| self.version(n))
            .or_else(|| {
                self.versions
                    .iter()
                    .find(|v| !v.yanked && !v.is_prerelease())
            })
            .or_else(|| self.versions.first())
    }

    /// Die Version, deren Features zu einer Versionsanforderung wie `"1.0"`
    /// oder `"^0.9"` gehören: die neueste passende (grob: gleiches Präfix).
    pub fn matching(&self, requirement: &str) -> Option<&CrateVersion> {
        let req = requirement.trim().trim_start_matches(['^', '~', '=', ' ']);
        if req.is_empty() || req == "*" {
            return self.recommended();
        }
        self.version(req).or_else(|| {
            self.versions
                .iter()
                .filter(|v| !v.yanked)
                .find(|v| v.num == req || v.num.starts_with(&format!("{req}.")))
        })
    }
}

/// Fehler beim Abruf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// Netzwerk nicht erreichbar, Zeitüberschreitung o. Ä.
    Network(String),
    /// Das Crate gibt es nicht.
    NotFound(String),
    /// Unerwartete Antwort.
    Invalid(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::Network(e) => write!(f, "Netzwerkfehler: {e}"),
            SourceError::NotFound(name) => write!(f, "Crate nicht gefunden: {name}"),
            SourceError::Invalid(e) => write!(f, "Ungültige Antwort: {e}"),
        }
    }
}

impl std::error::Error for SourceError {}

/// Eine Quelle für Crate-Informationen. `Send + Sync`, weil sie in
/// Hintergrund-Threads benutzt wird.
pub trait CrateSource: Send + Sync {
    fn search(&self, query: &str) -> Result<Vec<CrateSummary>, SourceError>;
    fn details(&self, name: &str) -> Result<CrateDetails, SourceError>;
}

/// Der echte Client für <https://crates.io>.
///
/// Hält sich an die Crawler-Regeln von crates.io: eindeutiger User-Agent
/// mit Kontakt, wenige Anfragen (der Editor sucht erst ab drei Zeichen und
/// mit Verzögerung nach der letzten Tastatureingabe).
pub struct CratesIo {
    agent: ureq::Agent,
    base_url: String,
    per_page: u32,
}

impl CratesIo {
    pub const USER_AGENT: &'static str = concat!(
        "ct-editor/",
        env!("CARGO_PKG_VERSION"),
        " (Freya TDE; https://github.com/Dragon-InterActive/Freya-TDE)"
    );

    pub fn new() -> Self {
        Self::with_base_url("https://crates.io")
    }

    /// Für Spiegelserver oder Tests.
    pub fn with_base_url(base_url: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(Self::USER_AGENT)
            .build()
            .into();
        Self {
            agent,
            base_url: base_url.trim_end_matches('/').to_owned(),
            per_page: 40,
        }
    }

    fn get<T: for<'de> Deserialize<'de>>(
        &self,
        request: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
        name: &str,
    ) -> Result<T, SourceError> {
        match request.call() {
            Ok(mut response) => response
                .body_mut()
                .with_config()
                .limit(64 * 1024 * 1024)
                .read_json::<T>()
                .map_err(|e| SourceError::Invalid(e.to_string())),
            Err(ureq::Error::StatusCode(404)) => Err(SourceError::NotFound(name.to_owned())),
            Err(e) => Err(SourceError::Network(e.to_string())),
        }
    }
}

impl Default for CratesIo {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    crates: Vec<ApiCrate>,
}

#[derive(Deserialize)]
struct ApiCrate {
    name: String,
    description: Option<String>,
    #[serde(default)]
    downloads: u64,
    default_version: Option<String>,
    max_stable_version: Option<String>,
    newest_version: Option<String>,
    homepage: Option<String>,
    repository: Option<String>,
    documentation: Option<String>,
}

#[derive(Deserialize)]
struct DetailsResponse {
    #[serde(rename = "crate")]
    krate: ApiCrate,
    #[serde(default)]
    versions: Vec<ApiVersion>,
}

#[derive(Deserialize)]
struct ApiVersion {
    num: String,
    #[serde(default)]
    yanked: bool,
    rust_version: Option<String>,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

impl CrateSource for CratesIo {
    fn search(&self, query: &str) -> Result<Vec<CrateSummary>, SourceError> {
        let request = self
            .agent
            .get(&format!("{}/api/v1/crates", self.base_url))
            .query("q", query.trim())
            .query("per_page", self.per_page.to_string());
        let response: SearchResponse = self.get(request, query)?;
        Ok(response
            .crates
            .into_iter()
            .map(|c| CrateSummary {
                version: c
                    .default_version
                    .or(c.max_stable_version)
                    .or(c.newest_version),
                description: non_empty(c.description),
                downloads: c.downloads,
                name: c.name,
            })
            .collect())
    }

    fn details(&self, name: &str) -> Result<CrateDetails, SourceError> {
        let request = self
            .agent
            .get(&format!("{}/api/v1/crates/{name}", self.base_url));
        let response: DetailsResponse = self.get(request, name)?;
        let c = response.krate;
        Ok(CrateDetails {
            name: c.name,
            description: non_empty(c.description),
            homepage: non_empty(c.homepage),
            repository: non_empty(c.repository),
            documentation: non_empty(c.documentation),
            default_version: c.default_version.or(c.max_stable_version),
            versions: response
                .versions
                .into_iter()
                .map(|v| CrateVersion {
                    num: v.num,
                    yanked: v.yanked,
                    rust_version: v.rust_version,
                    features: v.features,
                })
                .collect(),
        })
    }
}

/// Eine feste Liste von Crates – für Tests, Vorführungen und den
/// Offline-Betrieb (`--offline`: dann leer).
#[derive(Debug, Clone, Default)]
pub struct StaticSource {
    pub crates: Vec<CrateDetails>,
}

impl CrateSource for StaticSource {
    fn search(&self, query: &str) -> Result<Vec<CrateSummary>, SourceError> {
        let query = query.trim().to_lowercase();
        Ok(self
            .crates
            .iter()
            .filter(|c| c.name.contains(&query))
            .map(|c| CrateSummary {
                name: c.name.clone(),
                version: c.recommended().map(|v| v.num.clone()),
                description: c.description.clone(),
                downloads: 0,
            })
            .collect())
    }

    fn details(&self, name: &str) -> Result<CrateDetails, SourceError> {
        self.crates
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .ok_or_else(|| SourceError::NotFound(name.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(num: &str, features: &[(&str, &[&str])]) -> CrateVersion {
        CrateVersion {
            num: num.into(),
            yanked: false,
            rust_version: None,
            features: features
                .iter()
                .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
                .collect(),
        }
    }

    #[test]
    fn searchable_needs_three_chars() {
        assert!(!is_searchable("se"));
        assert!(!is_searchable("  se  "));
        assert!(is_searchable("ser"));
        assert!(is_searchable("äöü"));
    }

    #[test]
    fn default_features_ignore_dependencies() {
        let v = version(
            "1.0.0",
            &[("default", &["std", "dep:foo", "bar/baz"]), ("std", &[])],
        );
        assert_eq!(v.default_features(), ["std"]);
    }

    #[test]
    fn version_selection() {
        let details = CrateDetails {
            name: "x".into(),
            description: None,
            homepage: None,
            repository: None,
            documentation: None,
            default_version: None,
            versions: vec![
                version("2.0.0-beta.1", &[]),
                version("1.2.3", &[]),
                version("1.1.0", &[]),
                version("0.9.1", &[]),
            ],
        };
        assert_eq!(details.recommended().unwrap().num, "1.2.3");
        assert_eq!(details.matching("1").unwrap().num, "1.2.3");
        assert_eq!(details.matching("^1.1").unwrap().num, "1.1.0");
        assert_eq!(details.matching("0.9").unwrap().num, "0.9.1");
        assert_eq!(details.matching("*").unwrap().num, "1.2.3");
        assert!(details.matching("3").is_none());
    }

    #[test]
    fn api_json_is_understood() {
        let json = r#"{"crate":{"name":"demo","description":" Demo ","downloads":5,
            "default_version":"1.0.0","max_stable_version":"1.0.0","newest_version":"1.1.0-rc",
            "homepage":null,"repository":"https://x","documentation":""},
            "versions":[{"num":"1.0.0","yanked":false,"rust_version":"1.70",
            "features":{"default":["std"],"std":[]}}]}"#;
        let r: DetailsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.krate.name, "demo");
        assert_eq!(r.versions[0].features.len(), 2);
        assert_eq!(non_empty(r.krate.documentation), None);
    }

    /// Gegen das echte crates.io: `cargo test -- --ignored live_crates_io`.
    #[test]
    #[ignore = "braucht Netzwerk"]
    fn live_crates_io() {
        let client = CratesIo::new();
        let results = client.search("serde").unwrap();
        assert!(results.iter().any(|c| c.name == "serde"));
        let details = client.details("serde").unwrap();
        let recommended = details.recommended().unwrap();
        assert!(recommended.features.contains_key("derive"));
        assert_eq!(recommended.default_features(), ["std"]);
        assert!(matches!(
            client.details("dieses-crate-gibt-es-nicht-ct-editor"),
            Err(SourceError::NotFound(_))
        ));
    }
}
