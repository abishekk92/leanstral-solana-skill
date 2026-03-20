use super::templates;
use crate::ir::{ProofObligationIr, SupportedSurfaceIr};

pub struct PromptBuilder;

impl PromptBuilder {
    /// Build a complete prompt from IR
    pub fn build_prompt(
        source: &str,
        obligation: &ProofObligationIr,
        supported_surface: &SupportedSurfaceIr,
    ) -> String {
        let support_api = templates::support_api_for_modules(&obligation.lean_support_modules);
        let hint = templates::hint_for_category(&obligation.category);

        format!(
            "{preamble}\n\n\
            {common_patterns}\n\n\
            You MUST import the following support modules and use their definitions:\n{support_modules}\n\
            Write 'open Leanstral.Solana' at the top of your file.\n\n\
            ## Source Code\n\n```rust\n{source}\n```\n\n\
            ## Proof Obligation\n\n{title}\n\n\
            Category: {category}\n\
            Theorem Shape: {theorem_shape}\n\
            Relevant Instructions: {relevant_instructions}\n\n\
            Evidence:\n{evidence}\n\n\
            ## Supported Semantic Surface\n\n{surface}\n\n\
            ## Support API (Already Imported - DO NOT Redefine)\n\n\
            The following definitions are available after 'open Leanstral.Solana':\n\n\
            ```lean\n{support_api}\n```\n\n\
            ## Context\n\n{hint}\n\n\
            ## Theorem Skeleton (DO NOT MODIFY - Complete this exact signature)\n\n\
            ```lean\n{theorem_skeleton}\n```\n\n\
            {output_requirements}",
            preamble = templates::PREAMBLE.trim(),
            common_patterns = templates::COMMON_PATTERNS,
            source = source.trim(),
            title = obligation.title,
            category = obligation.category,
            theorem_shape = obligation.theorem_shape,
            relevant_instructions = obligation.relevant_instructions.join(", "),
            evidence = obligation
                .notes
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n"),
            support_modules = obligation
                .lean_support_modules
                .iter()
                .map(|module| format!("- import {module}"))
                .collect::<Vec<_>>()
                .join("\n"),
            surface = supported_surface
                .supported_property_categories
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n"),
            support_api = support_api,
            theorem_skeleton = obligation.theorem_skeleton,
            hint = hint,
            output_requirements = templates::OUTPUT_REQUIREMENTS,
        )
    }
}
