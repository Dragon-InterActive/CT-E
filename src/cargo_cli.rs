//! Prüfung durch Cargo selbst.
//!
//! `toml_edit` prüft die TOML-*Syntax* sofort beim Tippen. Ob das Manifest
//! auch für Cargo gültig ist (Pflichtfelder, gültige Edition, Workspace-
//! Bezüge, Schlüsselnamen …), weiß nur Cargo: dafür ruft der Editor
//! `cargo metadata --no-deps` auf, das nichts herunterlädt und nichts baut.

use std::path::Path;
use std::process::Command;

/// Ergebnis einer Prüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    /// Gültig; enthält ggf. Warnungen von Cargo.
    Valid { warnings: Vec<String> },
    /// Manifest in Ordnung, aber das Projekt hat noch keinen Quellcode
    /// (`src/main.rs`, `src/lib.rs`, `[lib]` oder `[[bin]]` fehlen). Kein Fehler
    /// der `Cargo.toml` – typisch für ein gerade angelegtes Projekt.
    NoTargets,
    /// Cargo lehnt das Manifest ab; enthält die Fehlermeldung.
    Invalid(String),
    /// Cargo konnte nicht gestartet werden.
    CargoMissing(String),
}

/// Das Cargo-Programm: `$CARGO` (gesetzt, wenn unter Cargo gestartet) oder `cargo`.
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Prüft die `Cargo.toml` unter `manifest_path` (blockierend).
pub fn check_manifest(manifest_path: &Path) -> CheckResult {
    let output = Command::new(cargo())
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest_path)
        .env("CARGO_TERM_COLOR", "never")
        .output();
    let output = match output {
        Ok(output) => output,
        Err(e) => return CheckResult::CargoMissing(e.to_string()),
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let warnings = stderr
            .lines()
            .filter(|l| l.starts_with("warning:"))
            .map(|l| l.trim_start_matches("warning:").trim().to_owned())
            .collect();
        CheckResult::Valid { warnings }
    } else if stderr.contains("no targets specified in the manifest") {
        CheckResult::NoTargets
    } else {
        CheckResult::Invalid(stderr.trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(name: &str, manifest: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ct-editor-cargo-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), "").unwrap();
        std::fs::write(dir.join("Cargo.toml"), manifest).unwrap();
        dir
    }

    #[test]
    fn valid_and_invalid_manifests() {
        let ok = temp_project(
            "ok",
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        assert!(matches!(
            check_manifest(&ok.join("Cargo.toml")),
            CheckResult::Valid { .. }
        ));

        let bad = temp_project(
            "bad",
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\nedition = \"1999\"\n",
        );
        match check_manifest(&bad.join("Cargo.toml")) {
            CheckResult::Invalid(message) => assert!(message.contains("edition"), "{message}"),
            other => panic!("{other:?}"),
        }

        let empty = temp_project("empty", "[package]\nname = \"x\"\nversion = \"0.1.0\"\n");
        std::fs::remove_dir_all(empty.join("src")).unwrap();
        assert_eq!(
            check_manifest(&empty.join("Cargo.toml")),
            CheckResult::NoTargets
        );

        std::fs::remove_dir_all(ok).ok();
        std::fs::remove_dir_all(empty).ok();
        std::fs::remove_dir_all(bad).ok();
    }
}
