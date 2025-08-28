# stackmint_amm

# 🚀 StackMint AMM Overview

`stackmint_amm` is a proof-of-concept constant-product automated market maker built with **Anchor** for the **Solana blockchain**.  
It supports a special "Stack" token (whose mint authority is a PDA), split fee models (protocol + creator), decimal normalization, and intuitive flows to mint or redeem Stack tokens directly through the AMM.

> ⚠️ **Note**: This project was developed as a proof of concept using the **Solana Playground IDE**.

---

## 🔍 Quick Summary

- **AMM Model**: Constant product (x * y = k) with normalized internal precision  
- **Fee Structure**: Protocol + Creator fees (configurable)  
- **PDAs**: `global`, `stack_info`, `pool`, `vault_authority`, `stack_mint_auth`  
- **Safety Features**: Reentrancy lock, pause switch, oracle guardrails, dust sweeping, and timelocked creator claims  
- **📡 Events**: Emitted for off-chain indexing (liquidity, swaps, claims, etc.)

---

## ⚙️ Important Constants (Updated)

| Constant                             | Description                                                                 |
|--------------------------------------|-----------------------------------------------------------------------------|
| `BPS_DENOM = 10_000`                 | Basis points denominator (1 bps = 1/10,000)                                 |
| `INTERNAL_PRECISION_DECIMALS = 9`    | Normalization base for internal `u128` arithmetic                          |
| `FALLBACK_DUST_THRESHOLD = 10`       | Tiny balances ≤ this (native units) are swept to treasury if unset in global |
| `FALLBACK_CREATOR_CLAIM_LOCK_SECS`   | 7-day fallback timelock if `global.creator_claim_lock_secs == 0`           |
| `FALLBACK_MAX_FEE_BPS = 2000`        | Max fee cap (20%) if `global.max_fee_bps` not set                          |

> 📝 `CREATOR_CLAIM_LOCK_SECS` constant was removed. The value is now dynamically read from `global.creator_claim_lock_secs`, with fallback.

---

## 🛠️ Entry Points (Instructions)

- `init_global`: Initializes global state with protocol settings and authority roles  
- `register_stack`: Registers a new Stack token and validates mint authority PDA  
- `create_pool`: Sets up an AMM pool with LP mint + vaults (token accounts owned by vault authority PDA)  
- `provide_liquidity`: Adds liquidity and mints LP tokens (fee-on-transfer supported)  
- `remove_liquidity`: Burns LP tokens and returns underlying assets  
- `swap_stack_to_quote` / `swap_quote_to_stack`: Swaps with fee logic, oracle price validation, and dust sweep  
- `mint_stack_via_pool` / `redeem_stack_via_pool`: Convenience wrappers to swap and mint/redeem Stack  
- `claim_creator_fees`: Allows creators to withdraw their fees after timelock  
- **Admin-only**: `emergency_pause`, `emergency_resume`, `withdraw_protocol_fees`, `set_pool_params`, `emergency_withdraw`

  ## 🧾 PDA Derivation

Seed patterns for deterministic PDA generation:

- `Global`: `["global"]`  
- `StackInfo`: `["stack_info", stack_mint]`  
- `StackMintAuth`: `["stack_mint_auth", stack_mint]`  
- `Pool`: `["pool", stack_mint, quote_mint]`  
- `VaultAuthority`: `["vault_authority", pool]`

---

## 🧮 Math & Helper Logic

All trading and fee operations use normalized **u128** math:

- `normalize_amount_u128`: Converts u64 native token → normalized u128  
- `denormalize_amount_u64`: Converts normalized u128 → native token u64 (floor)  
- `get_amount_out`: Standard constant-product formula  
- `integer_sqrt`: Used for initial LP token minting  

> ⚠️ Uses `checked_*` ops for overflow-safe math (returns `MathOverflow` error if invalid)

---

## 🛡️ Security & Safety Measures

- 🔒 **Reentrancy Lock**: Prevents nested state changes  
- ⏸️ **Pause Mechanism**: Global & per-pool pausing  
- 👮‍♂️ **PDA Ownership**: Vaults must be owned by vault_authority PDA  
- 🧮 **Oracle Check**: Enforces deviation bounds via `max_price_deviation_bps`  
- 💨 **Dust Sweeps**: Residual tokens below threshold routed to treasury  
- ⏳ **Timelocked Creator Fees**: Ensures fair claim delays  
- 🔑 **Role Access**: Admin, pauser, fee manager, governance controlled

---

## ❗ Error Codes

| Error Code              | Description & Fix |
|-------------------------|-------------------|
| `InvalidVaultOwner`     | Vault account not owned by PDA. Recreate ATA using correct PDA. |
| `InvalidVaultMint`      | Vault mint mismatch. Double-check the mint assigned to the vault. |
| `InvalidMintAuthority`  | PDA mismatch on stack mint. Confirm derivation order & seeds. |
| `MathOverflow`          | Decimal conversion or swap overflow. Review inputs and scaling. |
| `NoLiquidity`           | Swap/remove attempted on empty pool. Provide initial liquidity. |
| `Reentrancy`            | Nested operation blocked by lock. Avoid nested txs. |
| `OraclePriceMismatch`   | Price feed off by too much. Re-check oracle scaling & tolerance. |

---

## 🏗️ Program Structs

### 🧩 Global
- Admin roles + protocol config
- Treasury address for fee collection and dust sweeping

### 🧩 StackInfo
- Stack mint metadata, creator fee bps, optional rebalance hook

### 🧩 Pool
- All AMM state: paused flag, vaults, LP mint, oracle, fees, and creator balances

### 🧩 GovernanceApproval
- Optional stub for multisig governance

---

## 🧪 Testing & Diagnostics

- Log all PDA derivations and bumps  
- Ensure vault ATAs are owned by the `vault_authority` PDA  
- Print transaction logs (`getParsedTransaction`) to trace events and `msg!()`  
- Test edge cases:  
  - Fee-on-transfer tokens  
  - Mismatched decimals  
  - Oracle slippage rejections  
  - Reentrancy lock failures  

---

## ✅ Suggested Improvements / TODO

- Refactor: Consolidate duplicate fee logic  
- Tests: Add edge-case unit + fuzz tests  
- Safety: Add admin limits for max slippage, dynamic fee caps  
- Math: Migrate to fixed-point library (Q64, Q96-style) for better rounding

---


---
