//! Bettet unter Windows das Programmsymbol in die `.exe` ein (Explorer,
//! Taskleiste vor dem ersten Fenster); auf anderen Systemen nichts.

fn main() {
    println!("cargo:rerun-if-changed=assets/ct-editor.rc");
    println!("cargo:rerun-if-changed=assets/ct-editor.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/ct-editor.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("Windows-Ressource (Symbol) nicht übersetzbar");
    }
}
