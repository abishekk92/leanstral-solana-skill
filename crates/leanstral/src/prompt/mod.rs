// Prompt generation for Lean proof obligations
//
// This module is the single source of truth for all LLM prompts.
// It consumes structured IR from anchor-ir and generates prompts with:
// - Common tactic patterns (rewrite direction, etc.)
// - Category-specific guidance (access_control, conservation, etc.)
// - Support API documentation
// - Theorem skeletons

mod templates;
mod builder;

pub use builder::{PromptBuilder, ProofPlanIr};
