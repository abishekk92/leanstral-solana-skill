mod analyzer;
mod ir;

pub use analyzer::analyze_project;
pub use ir::{AnalysisIr, InstructionIr, PropertyCandidateIr, TestSignalIr};
