use crate::api::BuildStatus;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

pub struct ValidationResult {
    pub status: BuildStatus,
    pub log_path: Option<PathBuf>,
}

pub async fn validate_completion(
    output_dir: &Path,
    completion_index: usize,
    validation_workspace: Option<&Path>,
) -> Result<ValidationResult> {
    let log_dir = output_dir.join("validation");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join(format!("completion_{}.log", completion_index));

    let workspace = if let Some(ws) = validation_workspace {
        ws.to_path_buf()
    } else {
        ensure_validation_workspace().await?
    };

    // Copy Best.lean to validation workspace
    std::fs::copy(output_dir.join("Best.lean"), workspace.join("Best.lean"))?;

    // Set up Lake environment
    let lake_env = std::env::var("LAKE_ARTIFACT_CACHE").unwrap_or_else(|_| "true".to_string());

    // Run lake update
    let update_result = run_command(
        "lake",
        &["update", "leanstralSupport"],
        &workspace,
        &[("LAKE_ARTIFACT_CACHE", &lake_env)],
    )
    .await;

    match update_result {
        Ok((stdout, stderr, code)) => {
            if code != 0 {
                let combined = format!("{}\n{}", stdout, stderr);
                std::fs::write(&log_path, combined)?;
                return Ok(ValidationResult {
                    status: BuildStatus::Failed,
                    log_path: Some(log_path),
                });
            }
        }
        Err(e) => {
            std::fs::write(&log_path, format!("Error: {}", e))?;
            return Ok(ValidationResult {
                status: BuildStatus::Skipped,
                log_path: Some(log_path),
            });
        }
    }

    // Run lake build
    let build_result = run_command(
        "lake",
        &["--try-cache", "build", "Best"],
        &workspace,
        &[("LAKE_ARTIFACT_CACHE", &lake_env)],
    )
    .await;

    match build_result {
        Ok((stdout, stderr, code)) => {
            let combined = format!("{}\n{}", stdout, stderr);
            std::fs::write(&log_path, &combined)?;
            Ok(ValidationResult {
                status: if code == 0 {
                    BuildStatus::Success
                } else {
                    BuildStatus::Failed
                },
                log_path: Some(log_path),
            })
        }
        Err(e) => {
            std::fs::write(&log_path, format!("Error: {}", e))?;
            Ok(ValidationResult {
                status: BuildStatus::Skipped,
                log_path: Some(log_path),
            })
        }
    }
}

async fn run_command(
    cmd: &str,
    args: &[&str],
    cwd: &Path,
    env: &[(&str, &str)],
) -> Result<(String, String, i32)> {
    let mut command = Command::new(cmd);
    command.args(args).current_dir(cwd).stdout(Stdio::piped()).stderr(Stdio::piped());

    for (key, value) in env {
        command.env(key, value);
    }

    let output = command.output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    Ok((stdout, stderr, code))
}

async fn ensure_validation_workspace() -> Result<PathBuf> {
    let workspace = validation_workspace_dir()?;
    std::fs::create_dir_all(&workspace)?;
    crate::project::setup_lean_project(&workspace)?;
    Ok(workspace)
}

fn validation_workspace_dir() -> Result<PathBuf> {
    if let Ok(ws) = std::env::var("LEANSTRAL_VALIDATION_WORKSPACE") {
        return Ok(PathBuf::from(ws));
    }

    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(xdg)
            .join("leanstral-solana-skill")
            .join("validation-workspace"));
    }

    if cfg!(target_os = "macos") {
        let home = std::env::var("HOME")?;
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("leanstral-solana-skill")
            .join("validation-workspace"));
    }

    let home = std::env::var("HOME")?;
    Ok(PathBuf::from(home)
        .join(".cache")
        .join("leanstral-solana-skill")
        .join("validation-workspace"))
}

pub fn summarize_build_log(build_log: &str) -> String {
    let lines: Vec<&str> = build_log.lines().collect();
    let error_re = regex::Regex::new(r"\berror:|^error\b|unknown identifier|unexpected token").unwrap();

    let mut error_line_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| error_re.is_match(line))
        .map(|(idx, _)| idx)
        .collect();

    if !error_line_indices.is_empty() {
        // Take last 8 error lines
        error_line_indices = error_line_indices.into_iter().rev().take(8).rev().collect();

        let mut selected_indices = std::collections::HashSet::new();
        for idx in error_line_indices {
            for i in idx.saturating_sub(3)..=std::cmp::min(lines.len() - 1, idx + 6) {
                selected_indices.insert(i);
            }
        }

        let mut sorted_indices: Vec<usize> = selected_indices.into_iter().collect();
        sorted_indices.sort_unstable();

        let result: String = sorted_indices
            .iter()
            .map(|&i| lines[i])
            .collect::<Vec<_>>()
            .join("\n");

        return result.chars().take(24000).collect();
    }

    // Fallback: last 250 lines
    lines
        .iter()
        .rev()
        .take(250)
        .rev()
        .map(|s| *s)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(24000)
        .collect()
}

pub fn build_repair_prompt(
    original_prompt: &str,
    current_lean: &str,
    build_log: &str,
    round: usize,
) -> String {
    let summarized = summarize_build_log(build_log);
    format!(
        r#"You previously generated a Lean 4 proof module for this Solana verification task, but it did not compile.

Repair the Lean file using the compiler feedback below.

Hard requirements:
1. Return exactly one Lean 4 module.
2. Keep the same property target unless the build log proves it is impossible as stated.
3. Fix compiler errors concretely; do not leave declarations duplicated or theorem bodies empty.
4. Do not invent APIs or namespaces.
5. Prefer the smallest self-contained model that compiles under Lean 4.15 + Mathlib 4.15.
6. If a proof is incomplete, use `sorry` inside the proof body rather than leaving broken syntax.

This is repair round {}.

## Original Verification Task
{}

## Previous Lean Module
```lean
{}
```

## Lean Build Output
```
{}
```

## Repair Goal
Produce a revised Lean module that addresses the reported compiler errors and is more likely to pass `lake build Best`. Return Lean code only.
"#,
        round, original_prompt, current_lean, summarized
    )
}
