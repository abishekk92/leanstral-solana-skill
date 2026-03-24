// Lean proof generation IR types

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
