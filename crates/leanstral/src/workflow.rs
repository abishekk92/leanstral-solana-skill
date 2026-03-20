use crate::api::{generate_proofs, BuildStatus, LeanstralMetadata};
use crate::prompt::{PromptBuilder, ProofPlanIr};
use crate::validate::build_repair_prompt;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PropertyCandidate {
    id: String,
    category: String,
    title: String,
    confidence: String,
    relevant_instructions: Vec<String>,
    evidence: Vec<String>,
    prompt_hint: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnalysisIr {
    property_candidates: Vec<PropertyCandidate>,
}

fn candidate_priority(candidate: &PropertyCandidate) -> usize {
    let confidence_score = match candidate.confidence.as_str() {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    };

    let category_score = match candidate.category.as_str() {
        "access_control" => 0,
        "conservation" => 1,
        "state_machine" => 2,
        "arithmetic_safety" => 3,
        _ => 4,
    };

    confidence_score * 10 + category_score
}

fn select_candidates(candidates: Vec<PropertyCandidate>, top_k: usize) -> Vec<PropertyCandidate> {
    let mut ranked = candidates;
    ranked.sort_by_key(candidate_priority);

    let mut selected = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // First pass: maximize category coverage
    for category in &[
        "access_control",
        "conservation",
        "state_machine",
        "arithmetic_safety",
    ] {
        if let Some(candidate) = ranked.iter().find(|c| c.category == *category && !seen.contains(&c.id)).cloned() {
            selected.push(candidate.clone());
            seen.insert(candidate.id.clone());
            if selected.len() >= top_k {
                return selected;
            }
        }
    }

    // Second pass: fill remaining slots by overall rank
    for candidate in ranked {
        if seen.contains(&candidate.id) {
            continue;
        }
        selected.push(candidate.clone());
        seen.insert(candidate.id);
        if selected.len() >= top_k {
            break;
        }
    }

    selected
}

fn derive_project_root(paths: &[Option<PathBuf>]) -> Option<PathBuf> {
    let existing_paths: Vec<&PathBuf> = paths.iter().filter_map(|p| p.as_ref()).collect();
    if existing_paths.is_empty() {
        return None;
    }

    // Find common ancestor
    let first = existing_paths[0].canonicalize().ok()?;
    let mut ancestor = first.parent()?.to_path_buf();

    for path in &existing_paths[1..] {
        if let Ok(canonical) = path.canonicalize() {
            while !canonical.starts_with(&ancestor) {
                ancestor = ancestor.parent()?.to_path_buf();
            }
        }
    }

    // Remove /target suffix if present
    let ancestor_str = ancestor.to_string_lossy();
    if let Some(idx) = ancestor_str.rfind("/target") {
        return Some(PathBuf::from(&ancestor_str[..idx]));
    }

    Some(ancestor)
}

async fn attempt_repairs(
    candidate_id: &str,
    candidate_output_dir: &Path,
    original_prompt_file: &Path,
    repair_rounds: usize,
    passes: usize,
    temperature: f64,
    max_tokens: Option<usize>,
    validation_workspace: Option<&Path>,
) -> Result<()> {
    let metadata_path = candidate_output_dir.join("metadata.json");
    if !metadata_path.exists() {
        return Ok(());
    }

    let mut metadata: LeanstralMetadata = serde_json::from_str(&std::fs::read_to_string(&metadata_path)?)?;
    if metadata.best_selection_reason == "validated_build" {
        return Ok(());
    }

    let original_prompt = std::fs::read_to_string(original_prompt_file)?;

    for round in 1..=repair_rounds {
        let failed_completion = pick_best_failed_completion(&metadata);
        let Some(failed) = failed_completion else {
            eprintln!("No failed validated completion available to repair for {}.", candidate_id);
            return Ok(());
        };

        let Some(ref build_log_path) = failed.build_log_path else {
            eprintln!("No build log for failed completion in {}.", candidate_id);
            return Ok(());
        };

        let current_lean = std::fs::read_to_string(
            candidate_output_dir.join(format!("attempts/completion_{}.lean", failed.index)),
        )?;
        let build_log = std::fs::read_to_string(build_log_path)?;
        let repair_prompt = build_repair_prompt(&original_prompt, &current_lean, &build_log, round);

        let repair_prompt_file = candidate_output_dir.join(format!("repair_round_{}.prompt.txt", round));
        let repair_output_dir = candidate_output_dir.join(format!("repair_round_{}", round));

        std::fs::create_dir_all(&repair_output_dir)?;
        std::fs::write(&repair_prompt_file, &repair_prompt)?;

        eprintln!("Repairing {} (round {}/{})...", candidate_id, round, repair_rounds);

        generate_proofs(
            &repair_prompt,
            &repair_output_dir,
            passes,
            temperature,
            max_tokens.unwrap_or(16384),
            true, // always validate repairs
            validation_workspace,
        )
        .await?;

        let repaired_metadata_path = repair_output_dir.join("metadata.json");
        if !repaired_metadata_path.exists() {
            eprintln!("Repair metadata missing for {} on round {}.", candidate_id, round);
            return Ok(());
        }

        metadata = serde_json::from_str(&std::fs::read_to_string(&repaired_metadata_path)?)?;
        if metadata.best_selection_reason == "validated_build" {
            // Copy successful repair back
            std::fs::copy(
                repair_output_dir.join("Best.lean"),
                candidate_output_dir.join("Best.lean"),
            )?;
            std::fs::copy(&repaired_metadata_path, &metadata_path)?;
            eprintln!("Repair succeeded for {} on round {}.", candidate_id, round);
            return Ok(());
        }
    }

    Ok(())
}

fn pick_best_failed_completion(
    metadata: &LeanstralMetadata,
) -> Option<crate::api::CompletionMetadata> {
    let mut failed: Vec<_> = metadata
        .completions
        .iter()
        .filter(|c| c.build_status == BuildStatus::Failed && c.build_log_path.is_some())
        .cloned()
        .collect();

    failed.sort_by(|a, b| {
        if a.sorry_count != b.sorry_count {
            a.sorry_count.cmp(&b.sorry_count)
        } else {
            a.index.cmp(&b.index)
        }
    });

    failed.into_iter().next()
}

#[allow(clippy::too_many_arguments)]
pub async fn run_full_pipeline(
    idl: Option<PathBuf>,
    input: Option<PathBuf>,
    tests: Vec<PathBuf>,
    analysis_dir: PathBuf,
    output_dir: PathBuf,
    passes: usize,
    temperature: f64,
    max_tokens: Option<usize>,
    top_k: usize,
    validate: bool,
    repair_rounds: usize,
    analysis_only: bool,
) -> Result<()> {
    // Determine project root
    let paths = vec![idl.clone(), input.clone()];
    let project_root = derive_project_root(&paths);
    let validation_workspace = project_root.as_ref().map(|root| root.join(".leanstral/validation-workspace"));

    // Create directories
    std::fs::create_dir_all(&analysis_dir)?;
    std::fs::create_dir_all(&output_dir)?;

    // Run analyzer
    eprintln!("Analyzing Solana project...");
    anchor_ir::analyze_project(
        idl.as_deref(),
        input.as_deref(),
        &tests,
        Some(&analysis_dir),
    ).map_err(|e| anyhow::anyhow!(e))?;

    // Load analysis and proof plan
    let analysis: AnalysisIr = serde_json::from_str(&std::fs::read_to_string(analysis_dir.join("analysis.json"))?)?;
    let proof_plan: ProofPlanIr = serde_json::from_str(&std::fs::read_to_string(analysis_dir.join("proof_plan.json"))?)?;
    let ranked = select_candidates(analysis.property_candidates, top_k);

    // Read source code
    let source = if let Some(ref input_path) = input {
        std::fs::read_to_string(input_path)?
    } else {
        String::new()
    };

    // Output analysis summary
    let summary = serde_json::json!({
        "analysisDir": analysis_dir,
        "projectRoot": project_root,
        "selectedCandidates": ranked.iter().map(|c| {
            serde_json::json!({
                "id": c.id,
                "category": c.category,
                "confidence": c.confidence,
                "title": c.title,
                "promptFile": analysis_dir.join(format!("{}.prompt.txt", c.id)),
                "validationWorkspace": validation_workspace,
            })
        }).collect::<Vec<_>>(),
    });

    println!("{}", serde_json::to_string_pretty(&summary)?);

    if analysis_only {
        return Ok(());
    }

    // Generate proofs for each candidate
    for candidate in ranked {
        let candidate_output_dir = output_dir.join(&candidate.id);
        std::fs::create_dir_all(&candidate_output_dir)?;

        // Find the corresponding obligation from proof plan
        let obligation = proof_plan
            .obligations
            .iter()
            .find(|o| o.id == candidate.id)
            .ok_or_else(|| anyhow::anyhow!("No obligation found for candidate {}", candidate.id))?;

        // Generate prompt dynamically
        let prompt = PromptBuilder::build_prompt(&source, obligation, &proof_plan.supported_surface);

        // Save prompt for debugging/repair
        let prompt_file = candidate_output_dir.join("generated.prompt.txt");
        std::fs::write(&prompt_file, &prompt)?;

        eprintln!("Generating proof for {}...", candidate.id);
        match generate_proofs(
            &prompt,
            &candidate_output_dir,
            passes,
            temperature,
            max_tokens.unwrap_or(16384),
            validate,
            validation_workspace.as_deref(),
        )
        .await
        {
            Ok(_) => {
                // Attempt repairs if enabled
                if validate && repair_rounds > 0 {
                    attempt_repairs(
                        &candidate.id,
                        &candidate_output_dir,
                        &prompt_file,
                        repair_rounds,
                        passes,
                        temperature,
                        max_tokens,
                        validation_workspace.as_deref(),
                    )
                    .await?;
                }
            }
            Err(e) => {
                eprintln!("Generation failed for {}: {}", candidate.id, e);
                continue;
            }
        }
    }

    Ok(())
}
