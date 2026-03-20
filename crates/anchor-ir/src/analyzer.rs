use crate::ir::{
    AccountFieldIr, AccountsStructIr, AnalysisIr, ConstraintIr, InstructionIr, PropertyCandidateIr,
    TestSignalIr, TransferIr, CoverageSummaryIr, ProofObligationIr, ProofPlanIr,
    SupportedSurfaceIr, LlmQuery, LlmQuerySet, LlmResponse, LlmResponseSet, LlmParameter,
};
use anchor_syn::{AccountField, AccountsStruct, ConstraintGroup, Program, Ty};
use quote::ToTokens;
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{Attribute, File, Item};

pub fn analyze_project(
    idl_path: Option<&Path>,
    input: Option<&Path>,
    tests: &[PathBuf],
    output_dir: Option<&Path>,
) -> Result<String, String> {
    let source = if let Some(input) = input {
        Some(fs::read_to_string(input).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let parsed = if let Some(source) = &source {
        Some(syn::parse_file(source).map_err(|e| e.to_string())?)
    } else {
        None
    };

    let mut instructions = Vec::new();
    let mut accounts = Vec::new();

    if let Some(idl_path) = idl_path {
        let idl_source = fs::read_to_string(idl_path).map_err(|e| e.to_string())?;
        let idl: AnchorIdl = serde_json::from_str(&idl_source).map_err(|e| e.to_string())?;
        instructions = parse_idl_instructions(&idl);
        accounts = parse_idl_accounts(&idl);
    }

    if let Some(parsed) = &parsed {
        merge_instructions(&mut instructions, parse_program(parsed)?);
        merge_accounts(&mut accounts, parse_accounts(parsed)?);
    }

    let test_signals = parse_tests(tests)?;
    let property_candidates = derive_property_candidates(&instructions, &accounts, &test_signals);

    // Multi-turn LLM protocol: Check if we have LLM responses from a previous run
    let llm_responses = if let Some(output_dir) = output_dir {
        let response_path = output_dir.join("llm_responses.json");
        if response_path.exists() {
            Some(parse_llm_responses(&response_path)?)
        } else {
            None
        }
    } else {
        None
    };

    // Build proof plan with optional LLM enhancement
    let proof_plan = if let Some(responses) = &llm_responses {
        build_proof_plan_with_llm(&property_candidates, &instructions, &accounts, responses)
    } else {
        build_proof_plan(&property_candidates, &instructions, &accounts)
    };

    let ir = AnalysisIr {
        source_file: input
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        idl_file: idl_path.map(|path| path.display().to_string()),
        test_files: tests.iter().map(|p| p.display().to_string()).collect(),
        instructions: instructions.clone(),
        accounts,
        test_signals,
        property_candidates: property_candidates.clone(),
        proof_plan,
    };

    let json = serde_json::to_string_pretty(&ir).map_err(|e| e.to_string())?;
    if let Some(output_dir) = output_dir {
        fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
        fs::write(output_dir.join("analysis.json"), &json).map_err(|e| e.to_string())?;
        let proof_plan_json =
            serde_json::to_string_pretty(&ir.proof_plan).map_err(|e| e.to_string())?;
        fs::write(output_dir.join("proof_plan.json"), proof_plan_json)
            .map_err(|e| e.to_string())?;

        // Multi-turn LLM protocol: If we don't have responses yet, generate queries
        if llm_responses.is_none() {
            let queries = generate_llm_queries(
                &property_candidates,
                &instructions,
                source.as_deref().unwrap_or("")
            );

            if !queries.is_empty() {
                let query_set = LlmQuerySet {
                    version: "1.0".into(),
                    queries,
                };
                let queries_json = serde_json::to_string_pretty(&query_set)
                    .map_err(|e| e.to_string())?;
                fs::write(output_dir.join("llm_queries.json"), queries_json)
                    .map_err(|e| e.to_string())?;

                eprintln!("🤖 LLM assistance needed. Please analyze llm_queries.json and provide llm_responses.json");
                eprintln!("Query file: {}", output_dir.join("llm_queries.json").display());
                std::process::exit(2);
            }
        }

        Ok(format!(
            "Wrote analysis to {}",
            output_dir.join("analysis.json").display()
        ))
    } else {
        Ok(json)
    }
}

fn build_proof_plan(
    candidates: &[PropertyCandidateIr],
    instructions: &[InstructionIr],
    accounts: &[AccountsStructIr],
) -> ProofPlanIr {
    let mut unsupported_reasons = Vec::new();

    if instructions.iter().any(|ix| ix.transfers.iter().any(|t| t.uses_pda_signer)) {
        unsupported_reasons.push(
            "PDA signer semantics are approximated at the instruction level, not modeled from runtime semantics.".into(),
        );
    }

    if accounts.iter().any(|account| account.fields.iter().any(|field| field.ty.contains("AccountInfo"))) {
        unsupported_reasons.push(
            "Unchecked AccountInfo fields require conservative assumptions unless explicit invariants are extracted.".into(),
        );
    }

    let obligations: Vec<ProofObligationIr> = candidates
        .iter()
        .map(|candidate| ProofObligationIr {
            id: candidate.id.clone(),
            title: candidate.title.clone(),
            category: candidate.category.clone(),
            relevant_instructions: candidate.relevant_instructions.clone(),
            lean_support_modules: support_modules_for_category(&candidate.category),
            theorem_shape: theorem_shape_for_category(&candidate.category).into(),
            theorem_skeleton: generate_theorem_skeleton(candidate, instructions),
            status: "planned".into(),
            notes: candidate.evidence.clone(),
        })
        .collect();

    let unsupported_obligations = obligations
        .iter()
        .filter(|obligation| obligation.lean_support_modules.is_empty())
        .count();

    ProofPlanIr {
        supported_surface: SupportedSurfaceIr {
            lean_support_modules: vec![
                "Leanstral.Solana.Account".into(),
                "Leanstral.Solana.Authority".into(),
                "Leanstral.Solana.Token".into(),
                "Leanstral.Solana.State".into(),
            ],
            supported_property_categories: vec![
                "access_control".into(),
                "conservation".into(),
                "state_machine".into(),
                "arithmetic_safety".into(),
            ],
            unsupported_reasons,
        },
        coverage: CoverageSummaryIr {
            total_obligations: obligations.len(),
            supported_obligations: obligations.len().saturating_sub(unsupported_obligations),
            unsupported_obligations,
        },
        obligations,
    }
}

/// Build proof plan with LLM-enhanced theorem skeletons
fn build_proof_plan_with_llm(
    candidates: &[PropertyCandidateIr],
    instructions: &[InstructionIr],
    accounts: &[AccountsStructIr],
    llm_responses: &[LlmResponse],
) -> ProofPlanIr {
    let mut unsupported_reasons = Vec::new();

    if instructions.iter().any(|ix| ix.transfers.iter().any(|t| t.uses_pda_signer)) {
        unsupported_reasons.push(
            "PDA signer semantics are approximated at the instruction level, not modeled from runtime semantics.".into(),
        );
    }

    if accounts.iter().any(|account| account.fields.iter().any(|field| field.ty.contains("AccountInfo"))) {
        unsupported_reasons.push(
            "Unchecked AccountInfo fields require conservative assumptions unless explicit invariants are extracted.".into(),
        );
    }

    let obligations: Vec<ProofObligationIr> = candidates
        .iter()
        .map(|candidate| {
            // Find matching LLM response for this candidate
            let llm_response = llm_responses.iter()
                .find(|r| r.query_id == candidate.id);

            ProofObligationIr {
                id: candidate.id.clone(),
                title: candidate.title.clone(),
                category: candidate.category.clone(),
                relevant_instructions: candidate.relevant_instructions.clone(),
                lean_support_modules: support_modules_for_category(&candidate.category),
                theorem_shape: theorem_shape_for_category(&candidate.category).into(),
                theorem_skeleton: generate_enhanced_skeleton(candidate, instructions, llm_response),
                status: "planned".into(),
                notes: candidate.evidence.clone(),
            }
        })
        .collect();

    let unsupported_obligations = obligations
        .iter()
        .filter(|obligation| obligation.lean_support_modules.is_empty())
        .count();

    ProofPlanIr {
        supported_surface: SupportedSurfaceIr {
            lean_support_modules: vec![
                "Leanstral.Solana.Account".into(),
                "Leanstral.Solana.Authority".into(),
                "Leanstral.Solana.Token".into(),
                "Leanstral.Solana.State".into(),
            ],
            supported_property_categories: vec![
                "access_control".into(),
                "conservation".into(),
                "state_machine".into(),
                "arithmetic_safety".into(),
            ],
            unsupported_reasons,
        },
        coverage: CoverageSummaryIr {
            total_obligations: obligations.len(),
            supported_obligations: obligations.len().saturating_sub(unsupported_obligations),
            unsupported_obligations,
        },
        obligations,
    }
}

fn support_modules_for_category(category: &str) -> Vec<String> {
    match category {
        "access_control" => vec![
            "Leanstral.Solana.Account".into(),
            "Leanstral.Solana.Authority".into(),
        ],
        "conservation" => vec![
            "Leanstral.Solana.Account".into(),
            "Leanstral.Solana.Token".into(),
        ],
        "state_machine" => vec![
            "Leanstral.Solana.Account".into(),
            "Leanstral.Solana.State".into(),
        ],
        "arithmetic_safety" => vec!["Leanstral.Solana.Token".into()],
        _ => Vec::new(),
    }
}

fn theorem_shape_for_category(category: &str) -> &'static str {
    match category {
        "access_control" => "transition_non_none_implies_signer_equals_initializer",
        "conservation" => "direct_balance_equality_preserves_tracked_total",
        "state_machine" => "cancel_transition_sets_lifecycle_closed",
        "arithmetic_safety" => "transition_preserves_numeric_bounds",
        _ => "custom",
    }
}

fn generate_theorem_skeleton(
    candidate: &PropertyCandidateIr,
    instructions: &[InstructionIr],
) -> String {
    let default_name = String::from("transition");
    let instruction_name = candidate
        .relevant_instructions
        .first()
        .unwrap_or(&default_name);

    match candidate.category.as_str() {
        "access_control" => {
            format!(
                r#"theorem {}_access_control (p_preState : EscrowState) (p_signer : Pubkey)
    (h : {}Transition p_preState p_signer ≠ none) :
    p_signer = p_preState.initializer := by
  sorry"#,
                instruction_name, instruction_name
            )
        }
        "conservation" => {
            format!(
                r#"theorem {}_conservation (p_accounts p_accounts' : List Account)
    (h : {}PreservesBalances p_accounts = some p_accounts') :
    trackedTotal p_accounts = trackedTotal p_accounts' := by
  sorry"#,
                instruction_name, instruction_name
            )
        }
        "state_machine" => {
            format!(
                r#"theorem {}_closes_escrow (p_preState p_postState : EscrowState)
    (h : {}Transition p_preState = some p_postState) :
    p_postState.lifecycle = Lifecycle.closed := by
  sorry"#,
                instruction_name, instruction_name
            )
        }
        "arithmetic_safety" => {
            let args = instructions
                .iter()
                .find(|ix| ix.name == *instruction_name)
                .map(|ix| {
                    ix.args
                        .iter()
                        .filter(|arg| arg.contains("u64") || arg.contains("u8"))
                        .map(|arg| {
                            let parts: Vec<&str> = arg.split(':').collect();
                            format!("p_{}", parts[0].trim())
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();

            format!(
                r#"theorem {}_arithmetic_safety {} (p_preState p_postState : ProgramState)
    (h : {}Transition p_preState {} = some p_postState) :
    {} <= U64_MAX := by
  sorry"#,
                instruction_name,
                args,
                instruction_name,
                args,
                args.split_whitespace().next().unwrap_or("p_amount")
            )
        }
        _ => {
            format!(
                r#"theorem {}_property (p_s p_s' : ProgramState)
    (h : {}Transition p_s = some p_s') :
    true := by
  sorry"#,
                instruction_name, instruction_name
            )
        }
    }
}

/// Generate LLM queries for complex properties that need explicit parameter extraction
fn generate_llm_queries(
    candidates: &[PropertyCandidateIr],
    instructions: &[InstructionIr],
    source: &str,
) -> Vec<LlmQuery> {
    let mut queries = Vec::new();

    for candidate in candidates {
        // Conservation properties with transfers need LLM help to identify explicit parameters
        if candidate.category == "conservation" {
            if let Some(instruction) = instructions.iter()
                .find(|ix| candidate.relevant_instructions.contains(&ix.name))
            {
                if !instruction.transfers.is_empty() {
                    queries.push(LlmQuery {
                        id: candidate.id.clone(),
                        query_type: "conservation_parameters".into(),
                        instruction: instruction.name.clone(),
                        category: "conservation".into(),
                        transfers: instruction.transfers.clone(),
                        rust_code_snippet: extract_instruction_source(source, &instruction.name),
                        question: format!(
                            "This '{}' instruction performs {} token transfer(s). Analyze the transfers and provide:\n\
                            1. Explicit parameter names for all Pubkey authorities involved (from/to for each transfer)\n\
                            2. Explicit parameter names for transfer amounts\n\
                            3. Any distinctness constraints between authorities (e.g., from ≠ to)\n\
                            4. The complete theorem signature with ALL parameters explicitly declared\n\n\
                            Transfer details:\n{}\n\n\
                            Example response format:\n\
                            {{\n  \
                              \"query_id\": \"{}\",\n  \
                              \"parameters\": [\n    \
                                {{\"name\": \"p_from_authority\", \"param_type\": \"Pubkey\", \"description\": \"Authority of the from account\"}},\n    \
                                {{\"name\": \"p_to_authority\", \"param_type\": \"Pubkey\", \"description\": \"Authority of the to account\"}},\n    \
                                {{\"name\": \"p_amount\", \"param_type\": \"Nat\", \"description\": \"Transfer amount\"}}\n  \
                              ],\n  \
                              \"distinctness_constraints\": [\"p_from_authority ≠ p_to_authority\"],\n  \
                              \"theorem_signature\": \"theorem {}_conservation (p_accounts p_accounts' : List Account) (p_from_authority p_to_authority : Pubkey) (p_amount : Nat) (h_distinct : p_from_authority ≠ p_to_authority) (h : {}PreservesBalances p_accounts p_from_authority p_to_authority p_amount = some p_accounts') : trackedTotal p_accounts = trackedTotal p_accounts'\"\n\
                            }}",
                            instruction.name,
                            instruction.transfers.len(),
                            instruction.transfers.iter()
                                .enumerate()
                                .map(|(i, t)| format!(
                                    "  Transfer {}: from={:?}, to={:?}, authority={:?}, amount={:?}",
                                    i + 1, t.from, t.to, t.authority, t.amount_expr
                                ))
                                .collect::<Vec<_>>()
                                .join("\n"),
                            candidate.id,
                            instruction.name,
                            instruction.name
                        ),
                    });
                }
            }
        }
    }

    queries
}

/// Extract the source code for a specific instruction
fn extract_instruction_source(source: &str, instruction_name: &str) -> String {
    // Try to find the instruction function definition
    let pattern = format!(r"pub fn {}[^\{{]*\{{", instruction_name);
    if let Ok(re) = Regex::new(&pattern) {
        if let Some(mat) = re.find(source) {
            let start = mat.start();
            // Find the matching closing brace
            let remaining = &source[start..];
            if let Some(end) = find_matching_brace(remaining) {
                return source[start..start + end].to_string();
            }
        }
    }

    // Fallback: return a chunk around the instruction name
    if let Some(pos) = source.find(&format!("fn {}", instruction_name)) {
        let start = pos.saturating_sub(100);
        let end = (pos + 500).min(source.len());
        return source[start..end].to_string();
    }

    format!("// Could not extract source for instruction: {}", instruction_name)
}

/// Find the position of the matching closing brace
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.chars().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse LLM responses and use them to enhance theorem skeletons
fn parse_llm_responses(response_path: &Path) -> Result<Vec<LlmResponse>, String> {
    let content = fs::read_to_string(response_path)
        .map_err(|e| format!("Failed to read LLM responses: {}", e))?;

    let response_set: LlmResponseSet = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse LLM responses: {}", e))?;

    Ok(response_set.responses)
}

/// Generate an enhanced theorem skeleton using LLM-provided parameters
fn generate_enhanced_skeleton(
    candidate: &PropertyCandidateIr,
    instructions: &[InstructionIr],
    llm_response: Option<&LlmResponse>,
) -> String {
    // If we have LLM guidance for this obligation, use it
    if let Some(response) = llm_response {
        if let Some(signature) = &response.theorem_signature {
            return format!("{} := by\n  sorry", signature);
        }
    }

    // Otherwise fall back to the original skeleton generation
    generate_theorem_skeleton(candidate, instructions)
}

#[derive(Debug, Deserialize)]
struct AnchorIdl {
    #[serde(default)]
    instructions: Vec<IdlInstruction>,
    #[serde(default)]
    accounts: Vec<IdlAccountDef>,
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
    _writable: bool,
    #[serde(default)]
    pda: Option<IdlPda>,
}

#[derive(Debug, Deserialize)]
struct IdlPda {
    #[serde(default)]
    seeds: Vec<IdlSeed>,
}

#[derive(Debug, Deserialize)]
struct IdlSeed {
    #[serde(default)]
    _kind: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdlAccountDef {
    name: String,
    #[serde(default)]
    _discriminator: Vec<u8>,
}

fn parse_idl_instructions(idl: &AnchorIdl) -> Vec<InstructionIr> {
    idl.instructions
        .iter()
        .map(|ix| InstructionIr {
            name: ix.name.clone(),
            context_type: infer_context_type(&ix.name),
            args: ix
                .args
                .iter()
                .map(|arg| format!("{}: {}", arg.name, idl_type_label(&arg.ty)))
                .collect(),
            pda_seeds: ix
                .accounts
                .iter()
                .flat_map(|acct| acct.pda.iter())
                .flat_map(|pda| pda.seeds.iter())
                .filter_map(seed_label)
                .collect(),
            closes_accounts: Vec::new(),
            auth_signals: ix
                .accounts
                .iter()
                .filter(|acct| acct.signer)
                .map(|acct| format!("idl signer: {}", acct.name))
                .collect(),
            transfers: Vec::new(),
            evidence_sources: vec!["idl".into()],
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

fn parse_idl_accounts(idl: &AnchorIdl) -> Vec<AccountsStructIr> {
    idl.accounts
        .iter()
        .map(|account| AccountsStructIr {
            name: account.name.clone(),
            fields: Vec::new(),
            evidence_sources: vec!["idl".into()],
        })
        .collect()
}

fn infer_context_type(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn merge_instructions(existing: &mut Vec<InstructionIr>, incoming: Vec<InstructionIr>) {
    for instruction in incoming {
        if let Some(current) = existing.iter_mut().find(|ix| ix.name == instruction.name) {
            merge_string_vec(&mut current.args, instruction.args);
            merge_string_vec(&mut current.pda_seeds, instruction.pda_seeds);
            merge_string_vec(&mut current.closes_accounts, instruction.closes_accounts);
            merge_string_vec(&mut current.auth_signals, instruction.auth_signals);
            current.transfers.extend(instruction.transfers);
            merge_string_vec(&mut current.evidence_sources, instruction.evidence_sources);
            if current.context_type.is_empty() {
                current.context_type = instruction.context_type;
            }
        } else {
            existing.push(instruction);
        }
    }
}

fn merge_accounts(existing: &mut Vec<AccountsStructIr>, incoming: Vec<AccountsStructIr>) {
    for account in incoming {
        if let Some(current) = existing.iter_mut().find(|a| a.name == account.name) {
            current.fields = account.fields;
            merge_string_vec(&mut current.evidence_sources, account.evidence_sources);
        } else {
            existing.push(account);
        }
    }
}

fn merge_string_vec(current: &mut Vec<String>, incoming: Vec<String>) {
    let mut set: BTreeSet<String> = current.iter().cloned().collect();
    set.extend(incoming);
    *current = set.into_iter().collect();
}

fn parse_program(file: &File) -> Result<Vec<InstructionIr>, String> {
    let mut instructions = Vec::new();

    for item in &file.items {
        if let Item::Mod(item_mod) = item {
            if has_attr(&item_mod.attrs, "program") {
                let program: Program = syn::parse2(item_mod.to_token_stream())
                    .map_err(|e| format!("failed to parse #[program] module: {e}"))?;

                for ix in program.ixs {
                    let body = ix.raw_method.block.to_token_stream().to_string();
                    instructions.push(InstructionIr {
                        name: ix.ident.to_string(),
                        context_type: ix.anchor_ident.to_string(),
                        args: ix
                            .args
                            .iter()
                            .map(|arg| format!("{}: {}", arg.name, arg.raw_arg.ty.to_token_stream()))
                            .collect(),
                        pda_seeds: extract_seed_literals(&body),
                        closes_accounts: extract_close_targets(&body),
                        auth_signals: extract_auth_signals(&body),
                        transfers: extract_transfers(&body),
                        evidence_sources: vec!["rust".into()],
                    });
                }
            }
        }
    }

    Ok(instructions)
}

fn parse_accounts(file: &File) -> Result<Vec<AccountsStructIr>, String> {
    let mut accounts = Vec::new();

    for item in &file.items {
        if let Item::Struct(item_struct) = item {
            if has_derive_accounts(&item_struct.attrs) {
                let parsed: AccountsStruct = syn::parse2(item_struct.to_token_stream())
                    .map_err(|e| format!("failed to parse Accounts struct {}: {e}", item_struct.ident))?;
                accounts.push(AccountsStructIr {
                    name: parsed.ident.to_string(),
                    fields: parsed.fields.iter().map(account_field_to_ir).collect(),
                    evidence_sources: vec!["rust".into()],
                });
            }
        }
    }

    Ok(accounts)
}

fn parse_tests(test_files: &[PathBuf]) -> Result<Vec<TestSignalIr>, String> {
    let re = Regex::new(r#"(it|test)\s*\(\s*["'`](.*?)["'`]"#).map_err(|e| e.to_string())?;
    let mut signals = Vec::new();

    for test_file in test_files {
        let source = fs::read_to_string(test_file).map_err(|e| e.to_string())?;
        for capture in re.captures_iter(&source) {
            let name = capture.get(2).map(|m| m.as_str()).unwrap_or_default().to_string();
            signals.push(TestSignalIr {
                file: test_file.display().to_string(),
                inferred_properties: infer_properties_from_text(&name),
                name,
            });
        }
    }

    Ok(signals)
}

fn derive_property_candidates(
    instructions: &[InstructionIr],
    accounts: &[AccountsStructIr],
    test_signals: &[TestSignalIr],
) -> Vec<PropertyCandidateIr> {
    let mut candidates = Vec::new();
    let test_props: BTreeSet<String> = test_signals
        .iter()
        .flat_map(|signal| signal.inferred_properties.iter().cloned())
        .collect();

    for instruction in instructions {
        let account_struct = accounts
            .iter()
            .find(|account| account.name == instruction.context_type);

        let has_auth_constraint = account_struct.map(|account| {
            account.fields.iter().any(|field| {
                field.is_signer
                    || field
                        .constraints
                        .iter()
                        .any(|constraint| matches!(constraint.kind.as_str(), "has_one" | "owner" | "address"))
            })
        });
        let has_close_constraint = account_struct.map(|account| {
            account.fields.iter().any(|field| {
                field.constraints
                    .iter()
                    .any(|constraint| constraint.kind == "close")
            })
        });

        if has_auth_constraint.unwrap_or(false)
            || !instruction.auth_signals.is_empty()
            || test_props.contains("access_control")
        {
            candidates.push(PropertyCandidateIr {
                id: format!("{}_access_control", instruction.name),
                category: "access_control".into(),
                title: format!("{}: access control", instruction.name),
                confidence: "high".into(),
                relevant_instructions: vec![instruction.name.clone()],
                evidence: instruction.auth_signals.clone(),
                prompt_hint: format!(
                    "Model only the authorization condition for {}. Prove success implies the caller matches the required authority relation.",
                    instruction.name
                ),
            });
        }

        if !instruction.transfers.is_empty() || test_props.contains("conservation") {
            candidates.push(PropertyCandidateIr {
                id: format!("{}_conservation", instruction.name),
                category: "conservation".into(),
                title: format!("{}: token conservation", instruction.name),
                confidence: if instruction.transfers.len() > 1 {
                    "high".into()
                } else {
                    "medium".into()
                },
                relevant_instructions: vec![instruction.name.clone()],
                evidence: instruction
                    .transfers
                    .iter()
                    .map(|transfer| {
                        format!(
                            "transfer {:?} -> {:?} amount {:?}",
                            transfer.from, transfer.to, transfer.amount_expr
                        )
                    })
                    .collect(),
                prompt_hint: format!(
                    "Model only the balances touched by {}. Prove the total tracked amount is preserved across the transition.",
                    instruction.name
                ),
            });
        }

        if has_close_constraint.unwrap_or(false)
            || !instruction.closes_accounts.is_empty()
            || test_props.contains("state_machine")
        {
            candidates.push(PropertyCandidateIr {
                id: format!("{}_state_machine", instruction.name),
                category: "state_machine".into(),
                title: format!("{}: close / one-shot safety", instruction.name),
                confidence: "medium".into(),
                relevant_instructions: vec![instruction.name.clone()],
                evidence: instruction.closes_accounts.clone(),
                prompt_hint: format!(
                    "Model only the lifecycle flag for the state account. Prove {} moves it to a terminal closed state.",
                    instruction.name
                ),
            });
        }
    }

    if instructions
        .iter()
        .flat_map(|ix| ix.args.iter())
        .any(|arg| arg.contains("u8") || arg.contains("u16") || arg.contains("u32") || arg.contains("u64") || arg.contains("u128"))
        || test_props.contains("arithmetic_safety")
    {
        candidates.push(PropertyCandidateIr {
            id: "program_arithmetic_safety".into(),
            category: "arithmetic_safety".into(),
            title: "Program arithmetic safety".into(),
            confidence: "medium".into(),
            relevant_instructions: instructions.iter().map(|ix| ix.name.clone()).collect(),
            evidence: instructions
                .iter()
                .flat_map(|ix| ix.args.iter().cloned())
                .filter(|arg| arg.contains('u'))
                .collect(),
            prompt_hint:
                "Model only the numeric parameters and arithmetic preconditions that matter. Avoid token/account semantics unless required."
                    .into(),
        });
    }

    candidates
}

fn account_field_to_ir(field: &AccountField) -> AccountFieldIr {
    match field {
        AccountField::Field(field) => AccountFieldIr {
            name: field.ident.to_string(),
            ty: ty_to_string(&field.ty),
            is_signer: field.constraints.is_signer() || matches!(field.ty, Ty::Signer),
            is_mutable: field.constraints.is_mutable(),
            constraints: constraints_to_ir(&field.constraints),
        },
        AccountField::CompositeField(field) => AccountFieldIr {
            name: field.ident.to_string(),
            ty: field.symbol.clone(),
            is_signer: field.constraints.is_signer(),
            is_mutable: field.constraints.is_mutable(),
            constraints: constraints_to_ir(&field.constraints),
        },
    }
}

fn constraints_to_ir(group: &ConstraintGroup) -> Vec<ConstraintIr> {
    let mut constraints = Vec::new();

    if group.is_signer() {
        constraints.push(ConstraintIr {
            kind: "signer".into(),
            raw: "signer".into(),
            target: None,
        });
    }
    if group.is_mutable() {
        constraints.push(ConstraintIr {
            kind: "mut".into(),
            raw: "mut".into(),
            target: None,
        });
    }
    for has_one in &group.has_one {
        constraints.push(ConstraintIr {
            kind: "has_one".into(),
            raw: has_one.join_target.to_token_stream().to_string(),
            target: Some(has_one.join_target.to_token_stream().to_string()),
        });
    }
    if let Some(close) = &group.close {
        constraints.push(ConstraintIr {
            kind: "close".into(),
            raw: close.sol_dest.to_token_stream().to_string(),
            target: Some(close.sol_dest.to_token_stream().to_string()),
        });
    }
    if let Some(owner) = &group.owner {
        constraints.push(ConstraintIr {
            kind: "owner".into(),
            raw: owner.owner_address.to_token_stream().to_string(),
            target: Some(owner.owner_address.to_token_stream().to_string()),
        });
    }
    if let Some(address) = &group.address {
        constraints.push(ConstraintIr {
            kind: "address".into(),
            raw: address.address.to_token_stream().to_string(),
            target: Some(address.address.to_token_stream().to_string()),
        });
    }
    if let Some(seeds) = &group.seeds {
        constraints.push(ConstraintIr {
            kind: "seeds".into(),
            raw: seeds.seeds.to_token_stream().to_string(),
            target: None,
        });
    }
    if let Some(token) = &group.token_account {
        constraints.push(ConstraintIr {
            kind: "token_account".into(),
            raw: format!(
                "mint={}, authority={}",
                token.mint.to_token_stream(),
                token.authority.to_token_stream()
            ),
            target: None,
        });
    }
    if let Some(associated) = &group.associated_token {
        constraints.push(ConstraintIr {
            kind: "associated_token".into(),
            raw: format!(
                "mint={}, wallet={}",
                associated.mint.to_token_stream(),
                associated.wallet.to_token_stream()
            ),
            target: None,
        });
    }
    if let Some(init) = &group.init {
        constraints.push(ConstraintIr {
            kind: "init".into(),
            raw: format!("init(if_needed={})", init.if_needed),
            target: None,
        });
    }

    constraints
}

fn ty_to_string(ty: &Ty) -> String {
    match ty {
        Ty::Signer => "Signer".into(),
        Ty::UncheckedAccount => "UncheckedAccount".into(),
        Ty::AccountInfo => "AccountInfo".into(),
        Ty::SystemAccount => "SystemAccount".into(),
        Ty::ProgramData => "ProgramData".into(),
        Ty::Account(account) => account.account_type_path.to_token_stream().to_string(),
        Ty::LazyAccount(account) => account.account_type_path.to_token_stream().to_string(),
        Ty::AccountLoader(account) => account.account_type_path.to_token_stream().to_string(),
        Ty::Program(program) => program.account_type_path.to_token_stream().to_string(),
        Ty::Interface(interface) => interface.account_type_path.to_token_stream().to_string(),
        Ty::InterfaceAccount(account) => account.account_type_path.to_token_stream().to_string(),
        Ty::Sysvar(sysvar) => format!("{sysvar:?}"),
    }
}

fn extract_seed_literals(body: &str) -> Vec<String> {
    Regex::new(r#"b"([^"]+)""#)
        .unwrap()
        .captures_iter(body)
        .filter_map(|capture| capture.get(1).map(|m| m.as_str().to_string()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn extract_close_targets(body: &str) -> Vec<String> {
    Regex::new(r#"close\s*=\s*(\w+)"#)
        .unwrap()
        .captures_iter(body)
        .filter_map(|capture| capture.get(1).map(|m| m.as_str().to_string()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn extract_auth_signals(body: &str) -> Vec<String> {
    let mut signals = BTreeSet::new();
    for capture in Regex::new(r#"authority\s*:\s*ctx\s*\.\s*accounts\s*\.\s*(\w+)"#)
        .unwrap()
        .captures_iter(body)
    {
        if let Some(name) = capture.get(1) {
            signals.insert(format!("authority: {}", name.as_str()));
        }
    }
    if body.contains("initializer") {
        signals.insert("mentions initializer".into());
    }
    signals.into_iter().collect()
}

fn extract_transfers(body: &str) -> Vec<TransferIr> {
    let mut transfers = Vec::new();
    let from_re = Regex::new(r#"from\s*:\s*ctx\s*\.\s*accounts\s*\.\s*(\w+)"#).unwrap();
    let to_re = Regex::new(r#"to\s*:\s*ctx\s*\.\s*accounts\s*\.\s*(\w+)"#).unwrap();
    let authority_re = Regex::new(r#"authority\s*:\s*ctx\s*\.\s*accounts\s*\.\s*(\w+)"#).unwrap();
    let amount_re = Regex::new(r#"token\s*::\s*transfer\s*\(\s*[^,]+,\s*([A-Za-z0-9_\.]+)\s*\)"#).unwrap();

    let blocks: Vec<&str> = body.split("let cpi_accounts = Transfer").collect();
    for block in blocks.into_iter().skip(1) {
        transfers.push(TransferIr {
            from: from_re
                .captures(block)
                .and_then(|m| m.get(1).map(|v| v.as_str().to_string())),
            to: to_re
                .captures(block)
                .and_then(|m| m.get(1).map(|v| v.as_str().to_string())),
            authority: authority_re
                .captures(block)
                .and_then(|m| m.get(1).map(|v| v.as_str().to_string())),
            amount_expr: amount_re
                .captures(block)
                .and_then(|m| m.get(1).map(|v| v.as_str().to_string())),
            uses_pda_signer: block.contains("new_with_signer"),
        });
    }

    transfers
}

fn infer_properties_from_text(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut properties = Vec::new();

    if lower.contains("unauthorized")
        || lower.contains("permission")
        || lower.contains("only")
        || lower.contains("fail")
        || lower.contains("reject")
    {
        properties.push("access_control".into());
    }
    if lower.contains("balance") || lower.contains("transfer") || lower.contains("token") {
        properties.push("conservation".into());
    }
    if lower.contains("close") || lower.contains("reuse") || lower.contains("closed") {
        properties.push("state_machine".into());
    }
    if lower.contains("overflow") || lower.contains("underflow") || lower.contains("u64") {
        properties.push("arithmetic_safety".into());
    }

    properties
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path.is_ident(name))
}

fn has_derive_accounts(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path.is_ident("derive") {
            return false;
        }
        attr.to_token_stream().to_string().contains("Accounts")
    })
}
