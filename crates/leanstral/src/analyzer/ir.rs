use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisIr {
    pub source_file: String,
    pub idl_file: Option<String>,
    pub test_files: Vec<String>,
    pub instructions: Vec<InstructionIr>,
    pub test_signals: Vec<TestSignalIr>,
    pub property_candidates: Vec<PropertyCandidateIr>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstructionIr {
    pub name: String,
    pub args: Vec<String>,
    pub signers: Vec<String>,
    pub pda_seeds: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestSignalIr {
    pub file: String,
    pub name: String,
    pub inferred_properties: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PropertyCandidateIr {
    pub id: String,
    pub category: String,
    pub title: String,
    pub confidence: String,
    pub relevant_instructions: Vec<String>,
    pub evidence: Vec<String>,
    pub prompt_hint: String,
}
