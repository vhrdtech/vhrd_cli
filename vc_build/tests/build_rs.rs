//! The whole path: a scratch package with `vc_build::emit()` in its build.rs and `build_info!` in main prints the line.

use std::path::Path;
use std::process::Command;

#[test]
fn package_prints_its_version_line() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("vc_build-e2e");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    let lib = Path::new(env!("CARGO_MANIFEST_DIR")).display();
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"demo\"\nversion = \"1.2.3\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\nvc_build = {{ path = \"{lib}\" }}\n\n[build-dependencies]\nvc_build = {{ path = \"{lib}\" }}\n"
        ),
    )
    .unwrap();
    std::fs::write(root.join("build.rs"), "fn main() { vc_build::emit(); }\n").unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() { println!(\"{}\", vc_build::build_info!().line_named(\"demo\")); }\n",
    )
    .unwrap();

    // no repository here (the scratch dir is inside target/, so keep git from walking up to this checkout)
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args(["run", "-q", "--offline"]).current_dir(&root);
    for (k, _) in std::env::vars() {
        if k.starts_with("CARGO_") && k != "CARGO_HOME" {
            cmd.env_remove(k);
        }
    }
    let out = cmd
        .env("GIT_CEILING_DIRECTORIES", root.parent().unwrap())
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "demo 1.2.3 (unknown, commit unknown)"
    );
}
