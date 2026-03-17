import Lake
open Lake DSL

package «leanstral-proof» where
  version := "0.1.0"
  keywords := #["formal-verification", "leanstral"]
  leanOptions := #[
    ⟨`pp.unicode.fun, true⟩,
    ⟨`autoImplicit, false⟩
  ]

require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git"

@[default_target]
lean_lib «Best» where
  -- Builds best.lean (the top-ranked proof)
  roots := #[`Best]
