import Mathlib.Tactic
import Leanstral.Solana.Account
import Leanstral.Solana.Authority
import Leanstral.Solana.Cpi
import Leanstral.Solana.State
import Leanstral.Solana.Token
import Leanstral.Solana.Valid

open Leanstral.Solana

/- ============================================================================
   CancelAccessControl Proof
   ============================================================================ -/

namespace CancelAccessControl

structure EscrowState where
  initializer : Pubkey
  initializer_token_account : Pubkey
  initializer_amount : U64
  taker_amount : U64
  escrow_token_account : Pubkey
  bump : U8

def cancelTransition (p_preState : EscrowState) (p_signer : Pubkey) : Option Unit :=
  if p_signer = p_preState.initializer then
    some ()
  else
    none

theorem cancel_access_control (p_preState : EscrowState) (p_signer : Pubkey)
    (h : cancelTransition p_preState p_signer ≠ none) :
    p_signer = p_preState.initializer := by
  unfold cancelTransition at h
  split_ifs at h with h_eq
  · exact h_eq
  · contradiction

end CancelAccessControl

/- ============================================================================
   CancelCpiCorrectness Proof
   ============================================================================ -/

namespace CancelCpiCorrectness

structure CancelContext where
  escrow_token_account : Pubkey
  initializer_deposit : Pubkey
  authority : Pubkey
  amount : U64

def cancel_build_cpi (ctx : CancelContext) : TransferCpi :=
  { program := TOKEN_PROGRAM_ID
  , «from» := ctx.escrow_token_account
  , «to» := ctx.initializer_deposit
  , authority := ctx.authority
  , amount := ctx.amount }

theorem cancel_cpi_correct (ctx : CancelContext) :
    let cpi := cancel_build_cpi ctx
    cpi.program = TOKEN_PROGRAM_ID ∧
    cpi.«from» = ctx.escrow_token_account ∧
    cpi.«to» = ctx.initializer_deposit ∧
    cpi.authority = ctx.authority ∧
    cpi.amount = ctx.amount := by
  unfold cancel_build_cpi
  exact ⟨rfl, rfl, rfl, rfl, rfl⟩

end CancelCpiCorrectness

/- ============================================================================
   CancelStateMachine Proof
   ============================================================================ -/

namespace CancelStateMachine

structure EscrowState where
  lifecycle : Lifecycle
  initializer : Pubkey
  initializer_token_account : Pubkey
  initializer_amount : U64
  taker_amount : U64
  escrow_token_account : Pubkey
  bump : U8

def cancelTransition (p_preState : EscrowState) : Option EscrowState :=
  some { p_preState with lifecycle := Lifecycle.closed }

theorem cancel_closes_escrow (p_preState p_postState : EscrowState)
    (h : cancelTransition p_preState = some p_postState) :
    p_postState.lifecycle = Lifecycle.closed := by
  unfold cancelTransition at h
  cases h
  rfl

end CancelStateMachine

/- ============================================================================
   ExchangeAccessControl Proof
   ============================================================================ -/

namespace ExchangeAccessControl

structure EscrowState where
  initializer : Pubkey
  initializer_token_account : Pubkey
  initializer_amount : U64
  taker_amount : U64
  escrow_token_account : Pubkey
  bump : U8

def exchangeTransition (p_preState : EscrowState) (p_signer : Pubkey) : Option Unit :=
  if p_signer = p_preState.initializer then
    some ()
  else
    none

theorem exchange_access_control (p_preState : EscrowState) (p_signer : Pubkey)
    (h : exchangeTransition p_preState p_signer ≠ none) :
    p_signer = p_preState.initializer := by
  unfold exchangeTransition at h
  split_ifs at h with h_eq
  · exact h_eq
  · contradiction

end ExchangeAccessControl

/- ============================================================================
   ExchangeCpiCorrectness Proof
   ============================================================================ -/

namespace ExchangeCpiCorrectness

structure ExchangeContext where
  taker : Pubkey
  escrow : Pubkey
  taker_deposit : Pubkey
  initializer_receive : Pubkey
  escrow_token_account : Pubkey
  taker_receive : Pubkey
  taker_amount : U64
  initializer_amount : U64

def exchange_build_cpi_1 (ctx : ExchangeContext) : TransferCpi :=
  { program := TOKEN_PROGRAM_ID
  , «from» := ctx.taker_deposit
  , «to» := ctx.initializer_receive
  , authority := ctx.taker
  , amount := ctx.taker_amount }

def exchange_build_cpi_2 (ctx : ExchangeContext) : TransferCpi :=
  { program := TOKEN_PROGRAM_ID
  , «from» := ctx.escrow_token_account
  , «to» := ctx.taker_receive
  , authority := ctx.escrow
  , amount := ctx.initializer_amount }

theorem exchange_cpi_1_correct (ctx : ExchangeContext) :
    let cpi := exchange_build_cpi_1 ctx
    cpi.program = TOKEN_PROGRAM_ID ∧
    cpi.«from» = ctx.taker_deposit ∧
    cpi.«to» = ctx.initializer_receive ∧
    cpi.authority = ctx.taker ∧
    cpi.amount = ctx.taker_amount := by
  unfold exchange_build_cpi_1
  exact ⟨rfl, rfl, rfl, rfl, rfl⟩

theorem exchange_cpi_2_correct (ctx : ExchangeContext) :
    let cpi := exchange_build_cpi_2 ctx
    cpi.program = TOKEN_PROGRAM_ID ∧
    cpi.«from» = ctx.escrow_token_account ∧
    cpi.«to» = ctx.taker_receive ∧
    cpi.authority = ctx.escrow ∧
    cpi.amount = ctx.initializer_amount := by
  unfold exchange_build_cpi_2
  exact ⟨rfl, rfl, rfl, rfl, rfl⟩

end ExchangeCpiCorrectness

/- ============================================================================
   ExchangeStateMachine Proof
   ============================================================================ -/

namespace ExchangeStateMachine

structure EscrowState where
  lifecycle : Lifecycle
  initializer : Pubkey
  initializer_token_account : Pubkey
  initializer_amount : U64
  taker_amount : U64
  escrow_token_account : Pubkey
  bump : U8

def exchangeTransition (p_preState : EscrowState) : Option EscrowState :=
  some { p_preState with lifecycle := Lifecycle.closed }

theorem exchange_closes_escrow (p_preState p_postState : EscrowState)
    (h : exchangeTransition p_preState = some p_postState) :
    p_postState.lifecycle = Lifecycle.closed := by
  unfold exchangeTransition at h
  cases h
  rfl

end ExchangeStateMachine

/- ============================================================================
   InitializeAccessControl Proof
   ============================================================================ -/

namespace InitializeAccessControl

structure EscrowState where
  initializer : Pubkey
  initializer_token_account : Pubkey
  initializer_amount : U64
  taker_amount : U64
  escrow_token_account : Pubkey
  bump : U8

def initializeTransition (p_preState : EscrowState) (p_signer : Pubkey) : Option Unit :=
  if p_signer = p_preState.initializer then
    some ()
  else
    none

theorem initialize_access_control (p_preState : EscrowState) (p_signer : Pubkey)
    (h : initializeTransition p_preState p_signer ≠ none) :
    p_signer = p_preState.initializer := by
  unfold initializeTransition at h
  split_ifs at h with h_eq
  · exact h_eq
  · contradiction

end InitializeAccessControl

/- ============================================================================
   InitializeCpiCorrectness Proof
   ============================================================================ -/

namespace InitializeCpiCorrectness

structure InitializeContext where
  initializer : Pubkey
  initializer_deposit_token_account : Pubkey
  escrow_token_account : Pubkey
  amount : U64

def initialize_build_cpi (ctx : InitializeContext) : TransferCpi :=
  { program := TOKEN_PROGRAM_ID
  , «from» := ctx.initializer_deposit_token_account
  , «to» := ctx.escrow_token_account
  , authority := ctx.initializer
  , amount := ctx.amount }

theorem initialize_cpi_correct (ctx : InitializeContext) :
    let cpi := initialize_build_cpi ctx
    cpi.program = TOKEN_PROGRAM_ID ∧
    cpi.«from» = ctx.initializer_deposit_token_account ∧
    cpi.«to» = ctx.escrow_token_account ∧
    cpi.authority = ctx.initializer ∧
    cpi.amount = ctx.amount := by
  unfold initialize_build_cpi
  exact ⟨rfl, rfl, rfl, rfl, rfl⟩

end InitializeCpiCorrectness

/- ============================================================================
   ProgramArithmeticSafety Proof
   ============================================================================ -/

namespace ProgramArithmeticSafety

def U64_MAX : Nat := 18446744073709551615

structure ProgramState where
  amount : Nat
  taker_amount : Nat
  bump : Nat

def ValidState (s : ProgramState) : Prop :=
  s.amount <= U64_MAX ∧
  s.taker_amount <= U64_MAX ∧
  s.bump <= 255

def cancelTransition (p_s : ProgramState) : Option ProgramState :=
  some { p_s with amount := 0 }

theorem cancel_arithmetic_safety  (p_preState p_postState : ProgramState)
    (h : cancelTransition p_preState  = some p_postState) :
    p_postState.amount <= U64_MAX := by
  unfold cancelTransition at h
  cases h
  simp

end ProgramArithmeticSafety

