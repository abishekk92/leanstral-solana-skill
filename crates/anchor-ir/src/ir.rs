use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisIr {
    pub source_file: String,
    pub idl_file: Option<String>,
    pub test_files: Vec<String>,
    pub instructions: Vec<InstructionIr>,
    pub accounts: Vec<AccountsStructIr>,
    pub test_signals: Vec<TestSignalIr>,
    pub property_candidates: Vec<PropertyCandidateIr>,
}

/// Represents a precondition that must hold before an instruction executes
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreconditionIr {
    pub kind: PreconditionKind,
    pub description: String,
    pub source: String, // Where it was extracted from (constraint, test, doc)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PreconditionKind {
    /// Type bounds like "amount <= U64_MAX"
    TypeBound {
        variable: String,
        bound_type: String, // "u64", "u8", etc.
        upper_bound: String,
    },
    /// Balance checks like "initializer.balance >= amount"
    BalanceCheck {
        account: String,
        operator: String, // ">=", ">", etc.
        expression: String,
    },
    /// Authorization like "signer = initializer"
    Authorization {
        signer: String,
        expected: String,
    },
    /// State constraints like "escrow.lifecycle = open"
    StateConstraint {
        field: String,
        operator: String, // "=", "!=", etc.
        value: String,
    },
    /// Custom constraint expression
    Custom {
        expression: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstructionIr {
    pub name: String,
    pub context_type: String,
    pub args: Vec<String>,
    pub pda_seeds: Vec<String>,
    pub closes_accounts: Vec<String>,
    pub auth_signals: Vec<String>,
    pub transfers: Vec<TransferIr>,
    pub preconditions: Vec<PreconditionIr>,
    pub evidence_sources: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransferIr {
    pub from: Option<String>,
    pub to: Option<String>,
    pub authority: Option<String>,
    pub amount_expr: Option<String>,
    pub uses_pda_signer: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountsStructIr {
    pub name: String,
    pub fields: Vec<AccountFieldIr>,
    pub evidence_sources: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountFieldIr {
    pub name: String,
    pub ty: String,
    pub is_signer: bool,
    pub is_mutable: bool,
    pub constraints: Vec<ConstraintIr>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConstraintIr {
    pub kind: String,
    pub raw: String,
    pub target: Option<String>,
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
    pub preconditions: Vec<PreconditionIr>,
    pub prompt_hint: String,
}
