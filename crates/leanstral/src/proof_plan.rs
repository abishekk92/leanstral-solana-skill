// Proof planning for Lean verification
//
// This module builds Lean-specific proof plans from language-agnostic Anchor analysis IR.

use crate::ir::*;
use anchor_ir::{InstructionIr, AccountsStructIr, PropertyCandidateIr};
use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::Path;

/// Build a proof plan from property candidates
pub fn build_proof_plan(
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
pub fn build_proof_plan_with_llm(
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

/// Generate LLM queries for complex properties that need explicit parameter extraction
pub fn generate_llm_queries(
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

/// Parse LLM responses from a file
pub fn parse_llm_responses(response_path: &Path) -> Result<Vec<LlmResponse>> {
    let content = fs::read_to_string(response_path)?;
    let response_set: LlmResponseSet = serde_json::from_str(&content)?;
    Ok(response_set.responses)
}
