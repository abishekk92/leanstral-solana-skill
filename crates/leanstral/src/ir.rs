// Lean proof generation IR types
//
// This module contains types specific to the Lean proof generation workflow,
// including proof planning, obligations, and LLM interaction protocol.
// These types consume the language-agnostic IR from anchor-ir.

use serde::{Serialize, Deserialize};

// ============================================================================
// Proof Planning IR
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofPlanIr {
    pub supported_surface: SupportedSurfaceIr,
    pub obligations: Vec<ProofObligationIr>,
    pub coverage: CoverageSummaryIr,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupportedSurfaceIr {
    pub lean_support_modules: Vec<String>,
    pub supported_property_categories: Vec<String>,
    pub unsupported_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofObligationIr {
    pub id: String,
    pub title: String,
    pub category: String,
    pub relevant_instructions: Vec<String>,
    pub lean_support_modules: Vec<String>,
    pub theorem_shape: String,
    pub theorem_skeleton: String,
    pub status: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoverageSummaryIr {
    pub total_obligations: usize,
    pub supported_obligations: usize,
    pub unsupported_obligations: usize,
}

// ============================================================================
// LLM-Assisted Analysis Protocol
// ============================================================================

#[derive(Debug, Serialize)]
pub struct LlmQuery {
    pub id: String,
    pub query_type: String,
    pub instruction: String,
    pub category: String,
    pub transfers: Vec<anchor_ir::TransferIr>,
    pub rust_code_snippet: String,
    pub question: String,
}

#[derive(Debug, Serialize)]
pub struct LlmQuerySet {
    pub version: String,
    pub queries: Vec<LlmQuery>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmResponse {
    pub query_id: String,
    pub parameters: Vec<LlmParameter>,
    pub distinctness_constraints: Vec<String>,
    pub theorem_signature: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmParameter {
    pub name: String,
    pub param_type: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct LlmResponseSet {
    pub version: String,
    pub responses: Vec<LlmResponse>,
}
