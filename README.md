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
