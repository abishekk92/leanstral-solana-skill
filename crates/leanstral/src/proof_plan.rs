// Proof planning for Lean verification
//
// Builds Lean-specific proof plans from language-agnostic Anchor analysis IR.

use crate::ir::*;
use anchor_ir::{InstructionIr, PropertyCandidateIr};

/// Build a proof plan from property candidates
pub fn build_proof_plan(
    candidates: &[PropertyCandidateIr],
    instructions: &[InstructionIr],
) -> ProofPlanIr {
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
        .filter(|o| o.lean_support_modules.is_empty())
        .count();

    ProofPlanIr {
        supported_surface: SupportedSurfaceIr {
            lean_support_modules: vec![
                "Leanstral.Solana.Account".into(),
                "Leanstral.Solana.Authority".into(),
                "Leanstral.Solana.Token".into(),
                "Leanstral.Solana.State".into(),
                "Leanstral.Solana.Valid".into(),
            ],
            supported_property_categories: vec![
                "access_control".into(),
                "cpi_correctness".into(),
                "state_machine".into(),
                "arithmetic_safety".into(),
            ],
            unsupported_reasons: Vec::new(),
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
        "cpi_correctness" => vec![
            "Leanstral.Solana.Account".into(),
            "Leanstral.Solana.Cpi".into(),
        ],
        "state_machine" => vec![
            "Leanstral.Solana.Account".into(),
            "Leanstral.Solana.State".into(),
        ],
        "arithmetic_safety" => vec![
            "Leanstral.Solana.Account".into(),
            "Leanstral.Solana.Valid".into(),
        ],
        _ => Vec::new(),
    }
}

fn theorem_shape_for_category(category: &str) -> &'static str {
    match category {
        "access_control" => "transition_non_none_implies_signer_equals_initializer",
        "cpi_correctness" => "cpi_parameters_are_valid",
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
    let ix_name = candidate
        .relevant_instructions
        .first()
        .unwrap_or(&default_name);

    match candidate.category.as_str() {
        "access_control" => {
            format!(
                r#"theorem {ix}_access_control (p_preState : EscrowState) (p_signer : Pubkey)
    (h : {ix}Transition p_preState p_signer ≠ none) :
    p_signer = p_preState.initializer := by
  sorry"#,
                ix = ix_name
            )
        }
        "cpi_correctness" => {
            // CPI proofs are pure parameter mapping (all rfl).
            // Without source-level transfer info (pushed to coding agent),
            // emit a generic skeleton the LLM will adapt.
            format!(
                r#"theorem {ix}_cpi_correct (ctx : {ix}Context) :
    let cpi := {ix}_build_cpi ctx
    cpi.program = TOKEN_PROGRAM_ID ∧
    cpi.«from» = ctx.from_account ∧
    cpi.«to» = ctx.to_account ∧
    cpi.authority = ctx.authority ∧
    cpi.amount = ctx.amount := by
  unfold {ix}_build_cpi
  exact ⟨rfl, rfl, rfl, rfl, rfl⟩"#,
                ix = ix_name
            )
        }
        "state_machine" => {
            format!(
                r#"theorem {ix}_closes_escrow (p_preState p_postState : EscrowState)
    (h : {ix}Transition p_preState = some p_postState) :
    p_postState.lifecycle = Lifecycle.closed := by
  sorry"#,
                ix = ix_name
            )
        }
        "arithmetic_safety" => {
            let args = instructions
                .iter()
                .find(|ix| ix.name == *ix_name)
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
                r#"theorem {ix}_arithmetic_safety {args} (p_preState p_postState : ProgramState)
    (h : {ix}Transition p_preState {args} = some p_postState) :
    {bound_var} <= U64_MAX := by
  sorry"#,
                ix = ix_name,
                args = args,
                bound_var = args.split_whitespace().next().unwrap_or("p_amount")
            )
        }
        _ => {
            format!(
                r#"theorem {ix}_property (p_s p_s' : ProgramState)
    (h : {ix}Transition p_s = some p_s') :
    true := by
  sorry"#,
                ix = ix_name
            )
        }
    }
}
