import Leanstral.Solana
import Leanstral.Solana.Account
import Leanstral.Solana.Authority
import Leanstral.Solana.Cpi
import Leanstral.Solana.State
import Mathlib
import Mathlib.Tactic

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
  if h : p_signer = p_preState.initializer then
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
  initializer_deposit_token_account : Pubkey
  authority : Pubkey
  amount : U64

def cancel_build_transfer_cpi (p_ctx : CancelContext) : TransferCpi :=
  { program := TOKEN_PROGRAM_ID
  , «from» := p_ctx.escrow_token_account
  , «to» := p_ctx.initializer_deposit_token_account
  , authority := p_ctx.authority
  , amount := p_ctx.amount }

theorem cancel_cpi_valid (p_ctx : CancelContext)
    (p_distinct : p_ctx.escrow_token_account ≠ p_ctx.initializer_deposit_token_account)
    (p_amount : p_ctx.amount ≤ U64_MAX) :
    let cpi := cancel_build_transfer_cpi p_ctx
    transferCpiValid cpi ∧
    cpi.authority = p_ctx.authority ∧
    cpi.«from» ≠ cpi.«to» := by
  unfold cancel_build_transfer_cpi transferCpiValid
  exact ⟨⟨rfl, p_distinct, p_amount⟩, rfl, p_distinct⟩

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

def cancelTransition (p_s : EscrowState) : Option EscrowState :=
  some { p_s with lifecycle := Lifecycle.closed }

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
  taker_deposit_token_account : Pubkey
  initializer_receive_token_account : Pubkey
  escrow_token_account : Pubkey
  taker_receive_token_account : Pubkey
  amount1 : U64
  amount2 : U64

-- Build transfer CPIs for exchange instruction
def exchange_build_transfer_cpis (p_ctx : ExchangeContext) : List TransferCpi :=
  [ { program := TOKEN_PROGRAM_ID
    , «from» := p_ctx.taker_deposit_token_account
    , «to» := p_ctx.initializer_receive_token_account
    , authority := p_ctx.taker_deposit_token_account
    , amount := p_ctx.amount1 }
  , { program := TOKEN_PROGRAM_ID
    , «from» := p_ctx.escrow_token_account
    , «to» := p_ctx.taker_receive_token_account
    , authority := p_ctx.escrow_token_account
    , amount := p_ctx.amount2 } ]

theorem exchange_cpis_valid (p_ctx : ExchangeContext)
    (p_distinct1 : p_ctx.taker_deposit_token_account ≠ p_ctx.initializer_receive_token_account)
    (p_distinct2 : p_ctx.escrow_token_account ≠ p_ctx.taker_receive_token_account)
    (p_amount1 : p_ctx.amount1 ≤ U64_MAX)
    (p_amount2 : p_ctx.amount2 ≤ U64_MAX) :
    let cpis := exchange_build_transfer_cpis p_ctx
    multipleTransfersValid cpis ∧
    (∀ cpi ∈ cpis, cpi.program = TOKEN_PROGRAM_ID) := by
  unfold exchange_build_transfer_cpis
  unfold multipleTransfersValid
  simp only [Leanstral.Solana.transferCpiValid, Leanstral.Solana.Cpi.transferCpiValid]
  constructor
  · constructor
    · intro cpi h
      simp [List.mem_cons, List.mem_singleton] at h
      rcases h with rfl | rfl
      · exact ⟨rfl, p_distinct1, p_amount1⟩
      · exact ⟨rfl, p_distinct2, p_amount2⟩
    · intro cpi h
      simp [List.mem_cons, List.mem_singleton] at h
      rcases h with rfl | rfl
      · exact p_distinct1
      · exact p_distinct2
  · intro cpi h
    simp [List.mem_cons, List.mem_singleton] at h
    rcases h with rfl | rfl <;> rfl

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
  initializer_deposit_token_account : Pubkey
  escrow_token_account : Pubkey
  authority : Pubkey
  amount : U64

def initialize_build_transfer_cpi (p_ctx : InitializeContext) : TransferCpi :=
  { program := TOKEN_PROGRAM_ID
  , «from» := p_ctx.initializer_deposit_token_account
  , «to» := p_ctx.escrow_token_account
  , authority := p_ctx.authority
  , amount := p_ctx.amount }

theorem initialize_cpi_valid (p_ctx : InitializeContext)
    (p_distinct : p_ctx.initializer_deposit_token_account ≠ p_ctx.escrow_token_account)
    (p_amount : p_ctx.amount ≤ U64_MAX) :
    let cpi := initialize_build_transfer_cpi p_ctx
    transferCpiValid cpi ∧
    cpi.authority = p_ctx.authority ∧
    cpi.«from» ≠ cpi.«to» := by
  unfold initialize_build_transfer_cpi transferCpiValid
  exact ⟨⟨rfl, p_distinct, p_amount⟩, rfl, p_distinct⟩

end InitializeCpiCorrectness

