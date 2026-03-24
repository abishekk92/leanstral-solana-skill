---
name: leanstral
description: Use Mistral's Leanstral model to generate formal Lean 4 proofs for programs, especially Solana/Rust code. Trigger this skill whenever the user wants to formally verify code, generate Lean 4 proofs, prove properties about algorithms or smart contracts, verify invariants, convert program logic into formal specifications, or anything involving Lean 4 and formal verification. Also trigger when the user mentions "leanstral", "lean proof", "formal proof", "verify my code", "prove correctness", "formal verification", or wants mathematical guarantees about their implementation. Even if the user just says something like "prove this works" or "can you verify this function is correct", use this skill.
---

# Leanstral — Lean 4 Proof Generation via Mistral API

This skill calls Mistral's **Leanstral** model (`labs-leanstral-2603`) to generate formal Lean 4 proofs for Solana/Rust programs. The API is free during Mistral's feedback period.

## When to use this skill

- User wants to formally verify, prove properties about, or generate Lean 4 specifications for code
- User mentions "leanstral", "lean proof", "formal proof", "verify my code", "prove correctness"

## Workflow

### Step 1: Analyze and generate proofs

Use the `leanstral` CLI. The full pipeline (recommended):

```bash
leanstral verify \
  --idl path/to/target/idl/my_program.json \
  --input path/to/programs/my_program/src/lib.rs \
  --tests path/to/tests/my_program.ts \
  --output-dir /tmp/proofs \
  --top-k 3 \
  --repair-rounds 1 \
  --validate
```

This analyzes the program, ranks candidate properties, generates proofs via pass@N, validates with `lake build`, and retries on compiler errors.

Use `--analysis-only` to stop after property inference and prompt generation (no API calls).

For a single prompt:

```bash
leanstral generate \
  --prompt-file /tmp/prompt.txt \
  --output-dir /tmp/proof \
  --passes 4 \
  --validate
```

### Step 2: Evaluate results

For each completion:
1. Check that the theorem statement captures the intended property
2. Look for `sorry` markers (unfinished sub-proofs)
3. Prefer `lake build` success over heuristics — zero `sorry` doesn't guarantee compilation

Present the best completion. If `--validate` was used, the tool already selected the first compiling proof.

### Step 3: Iterate

- **`sorry` filling**: Ask Leanstral to fill specific `sorry` markers with the full proof context
- **Specification refinement**: Adjust theorem statements and re-prove
- **Property splitting**: One property per prompt works better than many at once
- **Tactic debugging**: Leanstral can diagnose why specific tactics fail

## Property priorities for Solana programs

Evidence precedence: IDL (structure) > Rust source (semantics) > tests (hints).

Property categories, in priority order:
1. **Access control** — signer checks, authority constraints
2. **CPI correctness** — correct parameters passed to each transfer (axiomatic, pure `rfl`)
3. **State machines** — lifecycle correctness, one-shot safety
4. **Arithmetic safety** — overflow/underflow for fixed-width integers

## Prompt structure

Prefer analyzer-generated prompts. For manual prompts:

```
I need a single Lean 4 module that compiles under Lean 4.15 + Mathlib 4.15.

Return Lean code only.
Do not duplicate theorem declarations.
If a proof is incomplete, use `sorry` inside the proof body.
Prefer a smaller model that compiles over a larger broken one.

## Source Code
<paste the relevant code here>

## Property to Prove
<state the property clearly>
```

One property at a time for nontrivial programs. Multiple theorems per prompt increases failure rates.

## Environment

- **`MISTRAL_API_KEY`** — required. Free from [console.mistral.ai](https://console.mistral.ai)
- **`LEANSTRAL_VALIDATION_WORKSPACE`** — optional override for the global Mathlib cache location

## Error handling

- **`MISTRAL_API_KEY` not set**: Direct user to console.mistral.ai
- **Rate limiting (429)**: Built-in exponential backoff handles this
- **Poor output**: Lower temperature to 0.3 or rephrase with more context
- **Long first build**: First Mathlib build takes 15-45 min; subsequent builds reuse the cache
- **Corrupt Mathlib**: Remove `.lake/packages/mathlib` and rerun
