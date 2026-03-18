use anyhow::Result;
use std::path::Path;

pub fn setup_lean_project(output_dir: &Path) -> Result<()> {
    // Copy template files from the repository
    let repo_root = std::env::current_dir()?;
    let templates_dir = repo_root.join("scripts/templates");
    let support_dir = repo_root.join("lean_support");

    // Copy lakefile.lean
    std::fs::copy(
        templates_dir.join("lakefile.lean"),
        output_dir.join("lakefile.lean"),
    )?;

    // Copy lean-toolchain
    std::fs::copy(
        templates_dir.join("lean-toolchain"),
        output_dir.join("lean-toolchain"),
    )?;

    // Copy Main.lean
    std::fs::copy(
        templates_dir.join("Main.lean"),
        output_dir.join("Main.lean"),
    )?;

    // Copy .gitignore
    std::fs::copy(
        templates_dir.join(".gitignore"),
        output_dir.join(".gitignore"),
    )?;

    // Copy README
    std::fs::copy(
        templates_dir.join("README.lean.md"),
        output_dir.join("README.md"),
    )?;

    // Copy lean_support directory
    copy_dir_recursive(&support_dir, &output_dir.join("lean_support"))?;

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
