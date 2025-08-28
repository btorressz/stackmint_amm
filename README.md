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

## ⚙️ Important Constants

| Constant                     | Description                                                                 |
|-----------------------------|-----------------------------------------------------------------------------|
| `BPS_DENOM = 10_000`        | Basis points denominator (1 bps = 1/10,000)                                 |
| `INTERNAL_PRECISION_DECIMALS = 9` | Normalization base for internal u128 arithmetic                    |
| `DUST_THRESHOLD = 10`       | Token units ≤ this threshold get swept to treasury                         |
| `CREATOR_CLAIM_LOCK_SECS`   | 7-day timelock before creators can claim accrued fees                      |

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

---
