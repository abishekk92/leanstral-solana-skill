import Leanstral.Solana.Account
import Leanstral.Solana.Authority

open Leanstral.Solana

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
  simp [cancelTransition] at h
  split_ifs at h with h_eq
  · exact h_eq
  · contradiction
