# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Leanstral is a Rust-based CLI tool for formally verifying Solana programs using Mistral's Leanstral model (labs-leanstral-2603). It analyzes Solana/Anchor programs, generates Lean 4 proof sketches via API calls, and validates them through compilation.

**Core workflow**: Rust source → Analyzer → Prompt generation → Leanstral API → Lean proofs → Lake validation

## Build and Development Commands

### Build the CLI

```bash
# Build leanstral binary (outputs to ./bin/leanstral)
cargo build --release

# Build just the Lean support library
cd crates/leanstral/lean_support
lake build
```

### Run Tests

```bash
# Test Lean support library axioms
cd crates/leanstral/lean_support
lake env lean test_lemmas.lean

# Build the example escrow verification
cd example/escrow
anchor build              # Build Solana program
npm install && npm test   # Run tests
cd formal_verification
lake build                # Verify all proofs compile
```

### Leanstral Commands

```bash
# Full verification pipeline (recommended)
./bin/leanstral verify \
  --idl example/escrow/target/idl/escrow.json \
  --input example/escrow/programs/escrow/src/lib.rs \
  --tests example/escrow/tests/escrow.ts \
  --output-dir /tmp/proofs \
  --top-k 3 \
  --validate \
  --repair-rounds 1

# Analysis only (extract properties without proof generation)
./bin/leanstral analyze \
  --input example/escrow/programs/escrow/src/lib.rs \
  --output-dir /tmp/analysis

# Generate proofs from existing prompt
./bin/leanstral generate \
  --prompt-file /tmp/analysis/property_name.prompt.txt \
  --output-dir /tmp/proof \
  --passes 3 \
  --temperature 0.3 \
  --validate

# Consolidate multiple proofs into single project
./bin/leanstral consolidate \
  --input-dir /tmp/proofs \
  --output-dir example/escrow/formal_verification
```

## Architecture

### Crate Structure

**`anchor-ir/`** - Analyzer for Solana/Anchor programs
- Parses IDL JSON (instructions, accounts, constraints)
- Extracts Rust source semantics (transfers, CPIs, closes)
- Parses test files for property hints
- Outputs: `AnalysisIr` with ranked property candidates

**`leanstral/`** - Main CLI and proof generation
- `main.rs` - CLI entry points (analyze, generate, verify, consolidate)
- `workflow.rs` - Full pipeline orchestration, candidate selection, repair loop
- `prompt/templates.rs` - Prompt templates with Lean tactics guidance
- `prompt/builder.rs` - Dynamic prompt construction from proof obligations
- `proof_plan.rs` - Converts property candidates to proof obligations
- `api.rs` - Mistral API client, pass@N support, retry logic
- `validate.rs` - Lake build validation, compiler error extraction
- `project.rs` - Lean project scaffolding generation
- `consolidate.rs` - Merges multiple proof projects

**`lean_support/`** - Canonical Lean axioms for Solana
- `Leanstral/Solana/Account.lean` - Account structure
- `Leanstral/Solana/Token.lean` - Token operations and conservation axioms
- `Leanstral/Solana/Authority.lean` - Authorization predicates
- `Leanstral/Solana/State.lean` - Lifecycle and state machines

### Data Flow

1. **Analysis Phase** (`anchor-ir`)
   - Input: IDL JSON, Rust source, test files
   - Output: `analysis.json` with `PropertyCandidateIr[]`
   - Emits: One `.prompt.txt` per property candidate

2. **Planning Phase** (`leanstral/proof_plan.rs`)
   - Converts `PropertyCandidateIr` → `ProofObligation`
   - Determines theorem signatures, state transitions, preconditions
   - Outputs: `proof_plan.json`

3. **Prompt Generation** (`leanstral/prompt/builder.rs`)
   - Builds prompt from `ProofObligation` + `SupportedSurface`
   - Includes: preamble, support API, Rust source, theorem skeleton

4. **Proof Generation** (`leanstral/api.rs`)
   - Calls Mistral API with pass@N sampling
   - Extracts Lean code blocks from completions
   - Selects best (fewest `sorry` markers, or validated build)

5. **Validation** (`leanstral/validate.rs`)
   - Copies completion to `Best.lean` in Lean project scaffold
   - Runs `lake build Best` in persistent workspace
   - Parses compiler errors for repair prompts

6. **Repair Loop** (`leanstral/workflow.rs`)
   - If validation fails and `--repair-rounds > 0`:
     - Build repair prompt from original + failed code + errors
     - Generate new pass@N completions
     - Validate again, repeat up to N rounds
   - On success: overwrite `Best.lean` with repaired version

### Key Design Decisions

**Why pass@N sampling?**
- Leanstral is non-deterministic; multiple attempts increase success rate
- Validation selects compilable proof over heuristics (sorry count)

**Why persistent validation workspace?**
- Lake's first `mathlib` build takes 15-45 minutes
- Reusing `.lake/packages/` avoids repeated Mathlib compilation
- Location: `<project_root>/.leanstral/validation-workspace`

**Why separate analysis and prompt generation?**
- Analyzer is language-agnostic (could support languages beyond Lean)
- Prompt templates are Lean-specific (`templates.rs`)
- Separation enables future backends (Coq, Isabelle, etc.)

**Why axioms instead of proving SPL Token?**
- Verification scope: program logic only (see VERIFICATION_SCOPE.md)
- Trust boundary: SPL Token, Solana runtime, CPI mechanics
- Pragmatic: keeps proofs tractable and completion time reasonable

## Verification Scope

**What we verify:**
- Authorization (signer checks, constraints)
- Conservation (token totals preserved)
- State machines (lifecycle, one-shot safety)
- Arithmetic safety (overflow/underflow)

**What we trust (axioms):**
- SPL Token implementation
- Solana runtime (PDA derivation, account ownership)
- CPI mechanics
- Anchor framework

See `example/escrow/formal_verification/VERIFICATION_SCOPE.md` for details.

## Common Development Tasks

### Improving Proof Generation Quality

Edit prompt templates in `crates/leanstral/src/prompt/templates.rs`:
- **PREAMBLE** - Initial context, goal, and constraints
- **SUPPORT_API** - Documents available axioms and lemmas
- **TACTICS** - Common Lean tactic patterns and gotchas
- **CONSERVATION_HINTS** - When to use which transfer lemma

After editing:
```bash
cargo build --release
./bin/leanstral verify ...  # Regenerate with new prompts
```

### Adding New Axioms

When a proof pattern is reusable across programs:

1. Add to `crates/leanstral/lean_support/Leanstral/Solana/Token.lean` (or other module)
2. Document the trust assumption with a comment
3. Export in `Leanstral.lean`
4. Update `templates.rs` SUPPORT_API section
5. Test: `cd crates/leanstral/lean_support && lake build`

### Debugging Failed Proofs

If `lake build` fails:
1. Check `metadata.json` for `build_status` and `build_log_path`
2. Read build log: `cat /tmp/proofs/<property_id>/build_log_*.txt`
3. Common issues:
   - `split_ifs` fails → use `unfold` before `split_ifs`
   - Cannot compose transfers → add composition lemma to `Token.lean`
   - Namespace collision → check `open` statements
4. Manually fix `Best.lean` or rerun with `--repair-rounds 2`

### Interactive Refinement Workflow

For complex properties:
```bash
# 1. Generate initial sketch
./bin/leanstral verify --input program.rs --output-dir /tmp/proofs --validate

# 2. Copy to project
cp -r /tmp/proofs/<property_id>/* example/my_program/formal_verification/

# 3. Iterate manually
cd example/my_program/formal_verification
lake build  # See errors
# Edit Best.lean based on errors
lake build  # Repeat until success
```

## Environment Variables

- `MISTRAL_API_KEY` - Required for proof generation
- `LEANSTRAL_VALIDATION_WORKSPACE` - Override validation workspace path (default: `<project_root>/.leanstral/validation-workspace`)
- `LAKE_ARTIFACT_CACHE=true` - Enable Lake's shared artifact cache

## Property Categories and Priority

Candidates are ranked by:
1. **Confidence**: high → medium → low
2. **Category**: access_control → conservation → state_machine → arithmetic_safety

Selection prioritizes category diversity (one of each) before filling by rank.

See `workflow.rs:10-64` for implementation.

## Common Lean Proof Patterns

### Tactic Sequencing
```lean
-- BAD: simp eliminates if-structure
simp [transition] at h
split_ifs at h  -- ERROR

-- GOOD: unfold preserves structure
unfold transition at h
split_ifs at h with h_eq
```

### Conservation Proofs
```lean
-- Single transfer
theorem transfer_conservation :=
  transfer_preserves_total ...

-- Two independent transfers (e.g., escrow exchange)
theorem exchange_conservation :=
  four_way_transfer_preserves_total ...

-- Balance updates with zero delta
theorem balance_conservation :=
  balance_update_preserves_total ...
```

### Equation Direction
```lean
-- If axiom gives: lhs = rhs
-- But you need: rhs = lhs
apply (transfer_preserves_total ...).symm
```

See `crates/leanstral/src/prompt/templates.rs` TACTICS section for more patterns.

## Output Artifacts

After `leanstral verify`:
```
/tmp/proofs/
├── analysis.json              # Ranked property candidates
├── proof_plan.json            # Proof obligations with signatures
├── <property_id>/
│   ├── Best.lean              # Selected best completion
│   ├── metadata.json          # Rankings, timings, tokens
│   ├── generated.prompt.txt   # Prompt sent to Leanstral
│   ├── attempts/
│   │   ├── completion_0.lean
│   │   ├── completion_0_raw.txt
│   │   └── ...
│   ├── build_log_0.txt        # Lake errors (if validation enabled)
│   └── repair_round_1/        # If repair attempted
│       ├── Best.lean
│       └── metadata.json
```

## Notes

- First Lean build is expensive (15-45 min for Mathlib). Subsequent builds are fast.
- If `lake build` fails with "could not resolve 'HEAD' to a commit", remove `.lake/packages/mathlib` and retry.
- Binary is built to `./bin/leanstral`, not `target/release/leanstral`.
- The tool reuses prompts across reruns; delete output dirs to force regeneration.
