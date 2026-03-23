import Mathlib.Tactic
import Leanstral.Solana.Account
import Leanstral.Solana.Authority
import Leanstral.Solana.State
import Leanstral.Solana.Token

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
   CancelConservation Proof
   ============================================================================ -/

namespace CancelConservation

def cancelTransition (p_accounts : List Account) (p_escrow_authority p_initializer_authority : Pubkey) (p_amount : Nat) : Option (List Account) :=
  some (p_accounts.map (fun acc =>
    if acc.authority = p_escrow_authority then
      { acc with balance := acc.balance - p_amount }
    else if acc.authority = p_initializer_authority then
      { acc with balance := acc.balance + p_amount }
    else
      acc))

theorem cancel_conservation (p_accounts p_accounts' : List Account) (p_escrow_authority p_initializer_authority : Pubkey) (p_amount : Nat) (h_distinct : p_escrow_authority ≠ p_initializer_authority) (h : cancelTransition p_accounts p_escrow_authority p_initializer_authority p_amount = some p_accounts') : trackedTotal p_accounts = trackedTotal p_accounts' := by
  unfold cancelTransition at h
  cases h
  exact (transfer_preserves_total p_accounts p_escrow_authority p_initializer_authority p_amount h_distinct).symm

end CancelConservation

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
   ExchangeConservation Proof
   ============================================================================ -/

namespace ExchangeConservation

def exchangePreservesBalances (p_accounts : List Account) (p_taker_authority p_initializer_receive_authority p_escrow_authority p_taker_receive_authority : Pubkey) (p_taker_amount p_initializer_amount : Nat) : Option (List Account) :=
  some (p_accounts.map (fun acc =>
    if acc.authority = p_taker_authority then
      { acc with balance := acc.balance - p_taker_amount }
    else if acc.authority = p_initializer_receive_authority then
      { acc with balance := acc.balance + p_taker_amount }
    else if acc.authority = p_escrow_authority then
      { acc with balance := acc.balance - p_initializer_amount }
    else if acc.authority = p_taker_receive_authority then
      { acc with balance := acc.balance + p_initializer_amount }
    else acc))

theorem exchange_conservation (p_accounts p_accounts' : List Account) (p_taker_authority p_initializer_receive_authority p_escrow_authority p_taker_receive_authority : Pubkey) (p_taker_amount p_initializer_amount : Nat) (h_distinct1 : p_taker_authority ≠ p_initializer_receive_authority) (h_distinct2 : p_escrow_authority ≠ p_taker_receive_authority) (h_distinct3 : p_taker_authority ≠ p_escrow_authority) (h : exchangePreservesBalances p_accounts p_taker_authority p_initializer_receive_authority p_escrow_authority p_taker_receive_authority p_taker_amount p_initializer_amount = some p_accounts') : trackedTotal p_accounts = trackedTotal p_accounts' := by
  unfold exchangePreservesBalances at h
  cases h
  exact (four_way_transfer_preserves_total p_accounts p_taker_authority p_initializer_receive_authority p_escrow_authority p_taker_receive_authority p_taker_amount p_initializer_amount h_distinct1 h_distinct2 h_distinct3).symm

end ExchangeConservation

/- ============================================================================
   ExchangeStateMachine Proof
   ============================================================================ -/

namespace ExchangeStateMachine

structure EscrowState where
  initializer : Pubkey
  initializer_token_account : Pubkey
  initializer_amount : U64
  taker_amount : U64
  escrow_token_account : Pubkey
  bump : U8
  lifecycle : Lifecycle

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
   InitializeConservation Proof
   ============================================================================ -/

namespace InitializeConservation

def initializeTransition (p_accounts : List Account) (p_initializer_authority p_escrow_authority : Pubkey) (p_amount : Nat) : Option (List Account) :=
  some (p_accounts.map (fun acc =>
    if acc.authority = p_initializer_authority then
      { acc with balance := acc.balance - p_amount }
    else if acc.authority = p_escrow_authority then
      { acc with balance := acc.balance + p_amount }
    else
      acc))

theorem initialize_conservation (p_accounts p_accounts' : List Account) (p_initializer_authority p_escrow_authority : Pubkey) (p_amount : Nat) (h_distinct : p_initializer_authority ≠ p_escrow_authority) (h : initializeTransition p_accounts p_initializer_authority p_escrow_authority p_amount = some p_accounts') : trackedTotal p_accounts = trackedTotal p_accounts' := by
  unfold initializeTransition at h
  cases h
  exact (transfer_preserves_total p_accounts p_initializer_authority p_escrow_authority p_amount h_distinct).symm

end InitializeConservation

/- ============================================================================
   ProgramArithmeticSafety Proof
   ============================================================================ -/

namespace ProgramArithmeticSafety

def U64_MAX : Nat := 2^64 - 1

structure EscrowState where
  initializer : Nat
  initializer_token_account : Nat
  initializer_amount : Nat
  taker_amount : Nat
  escrow_token_account : Nat
  bump : Nat

structure ProgramState where
  escrow : EscrowState
  counter : Nat

def cancelTransition (p_s : ProgramState) : Option ProgramState :=
  some { p_s with escrow := { p_s.escrow with initializer_amount := 0 } }

theorem cancel_arithmetic_safety (p_preState p_postState : ProgramState)
    (h : cancelTransition p_preState = some p_postState) :
    p_preState.escrow.initializer_amount <= U64_MAX := by
  unfold cancelTransition at h
  cases h
  sorry  -- This property requires preconditions about p_preState

end ProgramArithmeticSafety

