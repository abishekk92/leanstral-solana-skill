import Leanstral.Solana.Account
import Leanstral.Solana.Token

open Leanstral.Solana

def exchangePreservesBalances (p_accounts : List Account) : Option (List Account) :=
  match findByAuthority p_accounts (by sorry) with
  | none => none
  | some p_taker =>
    match findByAuthority p_accounts (by sorry) with
    | none => none
    | some p_initializer_receive =>
      match findByAuthority p_accounts (by sorry) with
      | none => none
      | some p_escrow =>
        some (p_accounts.map (fun acc =>
          if acc.authority = p_taker.authority then
            { acc with balance := acc.balance - p_escrow.balance }
          else if acc.authority = p_initializer_receive.authority then
            { acc with balance := acc.balance + p_escrow.balance }
          else if acc.authority = p_escrow.authority then
            { acc with balance := acc.balance + p_taker.balance }
          else acc))

theorem exchange_conservation (p_accounts p_accounts' : List Account)
    (h : exchangePreservesBalances p_accounts = some p_accounts') :
    trackedTotal p_accounts = trackedTotal p_accounts' := by
  rcases h with rfl
  exact transfer_preserves_total p_accounts (by sorry) (by sorry) (by sorry) (by sorry)
