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

### Step 1: Understand the program

Find and read the Anchor IDL JSON (typical location: `target/idl/<program_name>.json`). If it doesn't exist, tell the user to run `anchor build` first.

Read the IDL to understand the program's structure: instructions, accounts, arguments, and their types. Map these to user-facing functionality — don't present raw instruction names.

### Step 2: Build the verification scope interactively

Have a conversation with the user about what the program does and what matters. Ask about **functionality and risks**, not implementation details.

**Questions to ask:**

1. "What does this program do?" — Get the user's mental model in their words. An escrow program "lets two parties trade tokens safely" — that's the level to work at.
2. "What should never happen?" — This surfaces the critical safety properties. e.g., "tokens should never be lost", "only the depositor can withdraw", "a trade can't happen twice"
3. "What are you most worried about?" — Focus verification effort where the user perceives risk. Maybe they trust the happy path but worry about cancellation edge cases.
4. "Is there anything the program assumes but doesn't check?" — Surfaces implicit invariants that might not be in the code.

Don't ask about instructions, signers, or PDA seeds — you already have that from the IDL. Translate between the user's functional language and the technical structure yourself.

### Step 3: Write SPEC.md

Based on the conversation, create a `SPEC.md` in the project's `formal_verification/` directory. This is the contract between user intent and what gets verified.

Structure:

```markdown
# Verification Specification

## Program Summary
<1-2 sentences describing what the program does, in the user's language>

## Properties to Verify

### <Property name in plain language>
- **Category**: access_control | cpi_correctness | state_machine | arithmetic_safety
- **What it means**: <plain language description>
- **Why it matters**: <what could go wrong without this>
- **Instructions involved**: <which IDL instructions implement this>

### ...

## Out of Scope
<What we trust / don't verify and why>

## Trust Boundary
- SPL Token program (axiomatic)
- Solana runtime (PDA derivation, account ownership)
- Anchor framework (constraint enforcement)
```

Present SPEC.md to the user and get confirmation before proceeding. This is the most important step — wrong scope means wasted proofs.

### Step 4: Run analysis

```bash
leanstral verify \
  --idl path/to/target/idl/my_program.json \
  --validate \
  --analysis-only
```

Cross-reference `analysis.json` candidates against SPEC.md. Confirm the tool's candidates cover the user's stated properties. Adjust `--top-k` to match the number of properties in the spec.

Optional flags: `--input` (Rust source, passed to the LLM as context) and `--tests` (test files, hint signals for ranking).

### Step 5: Generate proofs

```bash
leanstral verify \
  --idl path/to/target/idl/my_program.json \
  --validate \
  --repair-rounds 1
```

### Step 6: Consolidate and verify

```bash
leanstral consolidate \
  --input-dir /tmp/leanstral-solana-proofs \
  --output-dir path/to/formal_verification
```

```bash
cd path/to/formal_verification
lake build
```

### Step 7: Report results against the spec

Map each generated proof back to the SPEC.md properties. Present results as:

- **Verified**: Theorem compiles, property proven
- **Partial**: Proof has `sorry` markers — sub-goals remain
- **Failed**: No compiling proof generated

For failures, iterate:
- **`sorry` filling**: Ask Leanstral to fill specific `sorry` markers
- **Specification refinement**: Adjust theorem statements and re-prove
- **Property splitting**: One property per prompt works better than many
- **Manual prompt**: Use `leanstral generate --prompt-file <file> --passes 4 --validate`

Update SPEC.md with results (which properties are proven, which remain open).

## Property categories

1. **Access control** — signer checks, authority constraints
2. **CPI correctness** — correct parameters passed to each transfer (axiomatic, pure `rfl`)
3. **State machines** — lifecycle correctness, one-shot safety
4. **Arithmetic safety** — overflow/underflow for fixed-width integers

## Environment

- **`MISTRAL_API_KEY`** — required. Free from [console.mistral.ai](https://console.mistral.ai)
- **`LEANSTRAL_VALIDATION_WORKSPACE`** — optional override for the global Mathlib cache location

## Error handling

- **`MISTRAL_API_KEY` not set**: Direct user to console.mistral.ai
- **Rate limiting (429)**: Built-in exponential backoff handles this
- **Poor output**: Lower temperature to 0.3 or rephrase with more context
- **Long first build**: First Mathlib build takes 15-45 min; subsequent builds reuse the cache
- **Corrupt Mathlib**: Remove `.lake/packages/mathlib` and rerun
