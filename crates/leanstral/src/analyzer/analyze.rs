use super::ir::{AnalysisIr, InstructionIr, PropertyCandidateIr, TestSignalIr};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn analyze_project(
    idl_path: Option<&Path>,
    _input: Option<&Path>,
    tests: &[PathBuf],
    output_dir: Option<&Path>,
) -> Result<String, String> {
    let mut instructions = Vec::new();
    let mut has_token_program: BTreeSet<String> = BTreeSet::new();
    let mut has_close_semantics: BTreeSet<String> = BTreeSet::new();

    if let Some(idl_path) = idl_path {
        let idl_source = fs::read_to_string(idl_path).map_err(|e| e.to_string())?;
        let idl: AnchorIdl = serde_json::from_str(&idl_source).map_err(|e| e.to_string())?;
        instructions = parse_idl_instructions(&idl);

        // Detect which instructions have token programs and close semantics
        for ix in &idl.instructions {
            let ix_has_token = ix.accounts.iter().any(|a| a.name.contains("token_program"));
            let ix_has_writable_pda = ix.accounts.iter().any(|a| a.writable && a.pda.is_some());
            let ix_has_relations = ix.accounts.iter().any(|a| !a.relations.is_empty());
            let ix_is_init = ix.name.contains("init");

            if ix_has_token {
                has_token_program.insert(ix.name.clone());
            }
            if ix_has_writable_pda && !ix_is_init && (ix_has_relations || ix.args.is_empty()) {
                has_close_semantics.insert(ix.name.clone());
            }
        }
    }

    let test_signals = parse_tests(tests)?;
    let property_candidates = derive_property_candidates(
        &instructions,
        &test_signals,
        &has_token_program,
        &has_close_semantics,
    );

    let ir = AnalysisIr {
        source_file: _input
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        idl_file: idl_path.map(|path| path.display().to_string()),
        test_files: tests.iter().map(|p| p.display().to_string()).collect(),
        instructions,
        test_signals,
        property_candidates,
    };

    let json = serde_json::to_string_pretty(&ir).map_err(|e| e.to_string())?;
    if let Some(output_dir) = output_dir {
        fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
        fs::write(output_dir.join("analysis.json"), &json).map_err(|e| e.to_string())?;
        Ok(format!(
            "Wrote analysis to {}",
            output_dir.join("analysis.json").display()
        ))
    } else {
        Ok(json)
    }
}

// ── IDL types ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AnchorIdl {
    #[serde(default)]
    instructions: Vec<IdlInstruction>,
}

#[derive(Debug, Deserialize)]
struct IdlInstruction {
    name: String,
    #[serde(default)]
    args: Vec<IdlArg>,
    #[serde(default)]
    accounts: Vec<IdlInstructionAccount>,
}

#[derive(Debug, Deserialize)]
struct IdlArg {
    name: String,
    #[serde(rename = "type")]
    ty: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct IdlInstructionAccount {
    name: String,
    #[serde(default)]
    signer: bool,
    #[serde(default)]
    writable: bool,
    #[serde(default)]
    pda: Option<IdlPda>,
    #[serde(default)]
    relations: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct IdlPda {
    #[serde(default)]
    seeds: Vec<IdlSeed>,
}

#[derive(Debug, Deserialize)]
struct IdlSeed {
    #[serde(default)]
    value: Option<serde_json::Value>,
    #[serde(default)]
    path: Option<String>,
}

// ── IDL parsing ─────────────────────────────────────────────────────────────

fn parse_idl_instructions(idl: &AnchorIdl) -> Vec<InstructionIr> {
    idl.instructions
        .iter()
        .map(|ix| InstructionIr {
            name: ix.name.clone(),
            args: ix
                .args
                .iter()
                .map(|arg| format!("{}: {}", arg.name, idl_type_label(&arg.ty)))
                .collect(),
            signers: ix
                .accounts
                .iter()
                .filter(|acct| acct.signer)
                .map(|acct| acct.name.clone())
                .collect(),
            pda_seeds: ix
                .accounts
                .iter()
                .flat_map(|acct| acct.pda.iter())
                .flat_map(|pda| pda.seeds.iter())
                .filter_map(seed_label)
                .collect(),
        })
        .collect()
}

fn seed_label(seed: &IdlSeed) -> Option<String> {
    if let Some(path) = &seed.path {
        return Some(path.clone());
    }
    match &seed.value {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Array(bytes)) => {
            let values: Vec<u8> = bytes
                .iter()
                .filter_map(|value| value.as_u64().and_then(|n| u8::try_from(n).ok()))
                .collect();
            String::from_utf8(values).ok().or_else(|| Some("const_seed".into()))
        }
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

fn idl_type_label(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ── Test parsing ────────────────────────────────────────────────────────────

fn parse_tests(test_paths: &[PathBuf]) -> Result<Vec<TestSignalIr>, String> {
    let mut signals = Vec::new();
    let test_re = Regex::new(r#"(?:it|test)\s*\(\s*["'`](.*?)["'`]"#).unwrap();

    for path in test_paths {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        for capture in test_re.captures_iter(&content) {
            if let Some(name) = capture.get(1) {
                let name = name.as_str().to_string();
                let inferred = infer_properties_from_text(&name);
                signals.push(TestSignalIr {
                    file: path.display().to_string(),
                    name,
                    inferred_properties: inferred,
                });
            }
        }
    }
    Ok(signals)
}

fn infer_properties_from_text(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut properties = Vec::new();
    if lower.contains("unauthorized") || lower.contains("permission") || lower.contains("reject") {
        properties.push("access_control".into());
    }
    if lower.contains("balance") || lower.contains("transfer") || lower.contains("token") {
        properties.push("cpi_correctness".into());
    }
    if lower.contains("close") || lower.contains("closed") {
        properties.push("state_machine".into());
    }
    if lower.contains("overflow") || lower.contains("underflow") {
        properties.push("arithmetic_safety".into());
    }
    properties
}

// ── Property candidate generation ───────────────────────────────────────────

fn derive_property_candidates(
    instructions: &[InstructionIr],
    test_signals: &[TestSignalIr],
    has_token_program: &BTreeSet<String>,
    has_close_semantics: &BTreeSet<String>,
) -> Vec<PropertyCandidateIr> {
    let mut candidates = Vec::new();
    let test_props: BTreeSet<String> = test_signals
        .iter()
        .flat_map(|signal| signal.inferred_properties.iter().cloned())
        .collect();

    for instruction in instructions {
        // Access control: instructions with signers
        if !instruction.signers.is_empty() || test_props.contains("access_control") {
            candidates.push(PropertyCandidateIr {
                id: format!("{}_access_control", instruction.name),
                category: "access_control".into(),
                title: format!("{}: access control", instruction.name),
                confidence: "high".into(),
                relevant_instructions: vec![instruction.name.clone()],
                evidence: instruction
                    .signers
                    .iter()
                    .map(|s| format!("signer: {}", s))
                    .collect(),
                prompt_hint: format!(
                    "Model the authorization condition for {}. Prove success implies the caller matches the required authority.",
                    instruction.name
                ),
            });
        }

        // CPI correctness: only instructions that actually have token_program
        if has_token_program.contains(&instruction.name) || test_props.contains("cpi_correctness") {
            candidates.push(PropertyCandidateIr {
                id: format!("{}_cpi_correctness", instruction.name),
                category: "cpi_correctness".into(),
                title: format!("{}: CPI parameter correctness", instruction.name),
                confidence: "medium".into(),
                relevant_instructions: vec![instruction.name.clone()],
                evidence: vec![],
                prompt_hint: format!(
                    "CPI calls are axiomatic. Verify that {} passes the correct parameters to each transfer CPI. Proof should be purely definitional (all rfl).",
                    instruction.name
                ),
            });
        }

        // State machine: only terminal operations (close semantics), not init
        if has_close_semantics.contains(&instruction.name) || test_props.contains("state_machine") {
            candidates.push(PropertyCandidateIr {
                id: format!("{}_state_machine", instruction.name),
                category: "state_machine".into(),
                title: format!("{}: close / one-shot safety", instruction.name),
                confidence: "medium".into(),
                relevant_instructions: vec![instruction.name.clone()],
                evidence: instruction
                    .pda_seeds
                    .iter()
                    .map(|s| format!("pda_seed: {}", s))
                    .collect(),
                prompt_hint: format!(
                    "Model the lifecycle flag for the state account. Prove {} closes the account.",
                    instruction.name
                ),
            });
        }

        // Arithmetic safety: per-instruction, only for instructions with numeric args
        let numeric_args: Vec<&String> = instruction.args.iter()
            .filter(|arg| {
                arg.contains("u8") || arg.contains("u16") || arg.contains("u32")
                    || arg.contains("u64") || arg.contains("u128")
            })
            .collect();

        if !numeric_args.is_empty() || test_props.contains("arithmetic_safety") {
            candidates.push(PropertyCandidateIr {
                id: format!("{}_arithmetic_safety", instruction.name),
                category: "arithmetic_safety".into(),
                title: format!("{}: arithmetic safety", instruction.name),
                confidence: "medium".into(),
                relevant_instructions: vec![instruction.name.clone()],
                evidence: numeric_args.iter().map(|a| (*a).clone()).collect(),
                prompt_hint: format!(
                    "Model the numeric parameters for {} and prove arithmetic bounds are preserved. Specific args: {}",
                    instruction.name,
                    numeric_args.iter().map(|a| a.as_str()).collect::<Vec<_>>().join(", ")
                ),
            });
        }
    }

    candidates
}
