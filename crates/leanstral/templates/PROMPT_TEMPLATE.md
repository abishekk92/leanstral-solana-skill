I need a single Lean 4 module that compiles under Lean 4.15 + Mathlib 4.15.

Return Lean code only.
Do not duplicate declarations.
Do not leave theorem bodies empty after `:= by`.
If a proof is incomplete, use `sorry` inside the proof body.
Prefer a smaller explicit model that compiles over a larger broken one.

Here is an example of a Rust function and the desired Lean proof structure. Follow this format.

---

### EXAMPLE

#### Rust Source
```rust
pub fn checked_add(a: u64, b: u64) -> Option<u64> {
    a.checked_add(b)
}
```

#### Lean Proof
```lean
import Mathlib.Data.Nat.Basic
import Mathlib.Tactic

def checked_add (a b : Nat) : Option Nat :=
  if h : a + b < 2^64 then
    some (a + b)
  else
    none

theorem checked_add_correct (a b : Nat) :
  a + b < 2^64 → checked_add a b = some (a + b) := by
  intro h
  simp [checked_add, h]

theorem checked_add_overflow (a b : Nat) :
  a + b ≥ 2^64 → checked_add a b = none := by
  intro h
  simp [checked_add, h]
```
---

Now, generate a Lean proof for the following.

## Source Code
<paste the relevant code here>

## Property to Prove
<state exactly one property, or a very small set of closely related properties>

## Context
<account model, invariants, semantic assumptions, or proof boundaries>

## Output Requirements
1. Define the model types and executable transition functions first.
2. State the theorem only after the semantics are defined.
3. Use only Lean 4.15 / Mathlib 4.15 identifiers you are confident exist.
4. Prefer concrete definitions over placeholders.
5. If the full request is too large, prove a sound subset cleanly instead of emitting broken code.
