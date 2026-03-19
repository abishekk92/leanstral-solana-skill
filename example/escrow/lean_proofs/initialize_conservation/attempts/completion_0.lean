import Leanstral.Solana.Account
import Leanstral.Solana.Token

open Leanstral.Solana

-- Define the transition function for initialize
def initializeTransition (p_accounts : List Account) (p_amount : Nat) : Option (List Account) :=
  match p_accounts with
  | [] => none
  | acc :: rest =>
    if acc.authority = p_accounts[1]!.authority then
      none
    else
      some (
        let new_acc := { acc with balance := acc.balance - p_amount }
        let new_acc' := { p_accounts[1]! with balance := p_accounts[1]!.balance + p_amount }
        new_acc :: new_acc' :: rest
      )

-- Helper lemma for the transition function
theorem initializeTransition_spec (p_accounts : List Account) (p_amount : Nat) :
    ∃ p_accounts', initializeTransition p_accounts p_amount = some p_accounts' := by
  refine ⟨[], ?_⟩
  simp [initializeTransition]

-- Main theorem about token conservation
theorem initialize_conservation (p_accounts p_accounts' : List Account) (p_amount : Nat)
    (h : initializeTransition p_accounts p_amount = some p_accounts') :
    trackedTotal p_accounts = trackedTotal p_accounts' := by
  rcases h with ⟨rfl⟩
  simp [initializeTransition, trackedTotal]
  rfl
