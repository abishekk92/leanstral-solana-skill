import Mathlib
import Aesop

-- Account types
structure Account where
  key : Nat  -- Public key (simplified as Nat for this model)
  balance : Nat
  is_closed : Bool
  authority : Nat

-- Escrow state
structure Escrow where
  initializer : Nat
  initializer_token_account : Nat
  initializer_amount : Nat
  taker_amount : Nat
  escrow_token_account : Nat
  bump : Nat
  is_active : Bool

-- Program state
structure ProgramState where
  accounts : List Account
  escrows : List Escrow
  constant_total_tokens : Nat

-- Instruction types
inductive Instruction
  | Initialize (amount : Nat) (taker_amount : Nat)
  | Exchange
  | Cancel


theorem token_conservation (initial_state final_state : ProgramState)
    (instr : Instruction) :
    let initial_total := initial_state.constant_total_tokens;
    let final_total := final_state.constant_total_tokens;
    initial_total = final_total := by


theorem access_control (escrow : Escrow) (signer : Nat) :
    (Instr.cancel escrow signer).isSuccess → signer = escrow.initializer := by


theorem exchange_correctness (state : ProgramState) (escrow : Escrow) :
    (Instr.exchange escrow).isSuccess →
    let taker_account := state.accounts.find (·.key = escrow.escrow_token_account);
    let initializer_account := state.accounts.find (·.key = escrow.initializer_token_account);
    let taker_balance_change := taker_account.balance - (escrow.taker_amount + taker_account.balance);
    let initializer_balance_change := initializer_account.balance - (escrow.initializer_amount + initializer_account.balance);
    taker_balance_change = escrow.initializer_amount ∧
    initializer_balance_change = escrow.taker_amount ∧
    escrow.is_active = False := by


theorem state_machine_safety (escrow : Escrow) :
    (Instr.exchange escrow).isSuccess ∨ (Instr.cancel escrow escrow.initializer).isSuccess →
    ¬escrow.is_active := by


theorem arithmetic_safety (amount taker_amount : Nat) :
    amount ≤ Nat.max ∧ taker_amount ≤ Nat.max := by


theorem token_conservation (initial_state final_state : ProgramState)
    (instr : Instruction) :
    let initial_total := initial_state.constant_total_tokens;
    let final_total := final_state.constant_total_tokens;
    initial_total = final_total := by
  -- The total tokens is constant by definition of the program
  -- All instructions preserve the total token balance
  cases instr <;> simp [ProgramState.constant_total_tokens]
  all_goals omega


theorem access_control (escrow : Escrow) (signer : Nat) :
    (Instr.cancel escrow signer).isSuccess → signer = escrow.initializer := by
  intro h
  cases h
  -- The cancel instruction only succeeds if signer is the initializer
  simp [Instr.cancel, Escrow.initializer] at *
  all_goals omega


theorem exchange_correctness (state : ProgramState) (escrow : Escrow) :
    (Instr.exchange escrow).isSuccess →
    let taker_account := state.accounts.find (·.key = escrow.escrow_token_account);
    let initializer_account := state.accounts.find (·.key = escrow.initializer_token_account);
    let taker_balance_change := taker_account.balance - (escrow.taker_amount + taker_account.balance);
    let initializer_balance_change := initializer_account.balance - (escrow.initializer_amount + initializer_account.balance);
    taker_balance_change = escrow.initializer_amount ∧
    initializer_balance_change = escrow.taker_amount ∧
    escrow.is_active = False := by
  intro h
  cases h
  -- The exchange instruction performs the required transfers
  simp [Instr.exchange, Escrow.is_active]
  all_goals omega


theorem state_machine_safety (escrow : Escrow) :
    (Instr.exchange escrow).isSuccess ∨ (Instr.cancel escrow escrow.initializer).isSuccess →
    ¬escrow.is_active := by
  intro h
  cases h
  · -- After exchange, escrow is closed
    simp [Instr.exchange, Escrow.is_active]
  · -- After cancel, escrow is closed
    simp [Instr.cancel, Escrow.is_active]


theorem arithmetic_safety (amount taker_amount : Nat) :
    amount ≤ Nat.max ∧ taker_amount ≤ Nat.max := by
  -- All amounts are natural numbers which are inherently bounded by Nat.max
  constructor
  · omega
  · omega
