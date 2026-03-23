mod analyzer;
mod ir;

pub use analyzer::analyze_project;
pub use ir::{
    AccountFieldIr, AccountsStructIr, AnalysisIr, ConstraintIr, InstructionIr,
    PreconditionIr, PreconditionKind, PropertyCandidateIr, TestSignalIr, TransferIr,
};
