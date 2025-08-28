# StackMint AMM 🪙⚖️

## Overview

**StackMint AMM** is a Solana program built with [Anchor](https://book.anchor-lang.com/) that provides a robust Automated Market Maker (AMM) for token pairs, supporting advanced features such as customizable fees, creator rewards, dust sweeping, and governance hooks. The protocol is highly configurable for both protocol administrators and pool creators, with safety and flexibility top of mind.

**NOTE** this project is a proof of concept/prototype that was developed in Solana Playground IDE

---

## ✨ Features

- **Automated Market Maker** for token swaps (constant product curve)
- **Liquidity Provision & Removal** with canonical LP tokens
- **Fee Routing**: Protocol, creator, and pool fees
- **Governance-Secured Actions**: Optional off-chain multisig approvals for sensitive operations
- **Oracle Price Checks**: Slippage and price protection via oracles (e.g., Pyth could be added in production or v3 PoC)
- **Configurable Dust Thresholds**: Automatic sweeping of small balances to treasury
- **Emergency Pause & Withdrawals**: Admin/pauser controls for system safety
- **Permissioned Creator Rewards**: Claimable after a configurable time lock
- **Upgradeable & Versioned State** for future-proofing

---

## 📦 Program Instructions

### 1. Initialize Global State

- Set protocol-wide roles (admin, pauser, governance)
- Configure fee caps, dust threshold, claim lock, etc.

### 2. Register Stack (Token)

- Register a new token mint for use in pools
- Assign creator and creator fee rate

### 3. Create Pool

- Establish a pool for a stack/quote token pair
- Set pool-specific parameters: fee, normalization decimals, vault accounts

### 4. Provide Liquidity

- Deposit both tokens
- Receive LP tokens based on sqrt(x*y) invariant

### 5. Remove Liquidity

- Burn LP tokens to withdraw underlying assets (pro-rata)

### 6. Swap Tokens

- Swap in either direction (stack -> quote or quote -> stack)
- Fees routed automatically, slippage/oracle checks available

### 7. Mint/Redeem Stack via Pool

- Mint: Swap quote for stack (stack mint authority PDA must sign)
- Redeem: Burn stack for quote payout

### 8. Claim Creator Fees

- Creators can claim accumulated fees after a time lock

### 9. Admin Functions

- **Pause/Resume** protocol
- **Emergency Withdraw**: LPs can exit even when paused
- **Withdraw Protocol Fees**: Admin, fee manager, or governance can withdraw

### 10. Governance Approvals

- Optional off-chain multisig PDA approvals for sensitive actions
- Replay protection using nonces

---

## 🛡️ Safety & Design Highlights

- **Reentrancy Protection**: Pools are locked during mutating operations
- **Math Overflow Checks**: All arithmetic is checked for overflows
- **Configurable Fee Caps**: Global and per-pool max fee BPS
- **Oracle Price Protection**: Prevents excessive slippage or price manipulation
- **Dust Handling**: Small residuals automatically swept to treasury

---

## 🛠️ Tech Stack

- **Rust** + **Anchor** framework
- **Solana SPL Token** program interface
- **Anchor Events** for off-chain tracking
- **Upgradeable Program Accounts** with version tracking

---

## 📋 Example Usage

```rust
// Provide Liquidity
stackmint_amm::provide_liquidity(ctx, amount_stack, amount_quote)?;
// Swap Stack to Quote
stackmint_amm::swap_stack_to_quote(ctx, amount_in, min_out, oracle_price, use_governance)?;
// Mint Stack via Pool
stackmint_amm::mint_stack_via_pool(ctx, quote_in, min_stack_out, oracle_price)?;
```

---

## 📝 Error Handling

All errors are well-documented and use descriptive messages (see `AmmError` enum):

- `InvalidFee`
- `ProtocolPaused`
- `MathOverflow`
- `SlippageExceeded`
- `Unauthorized`
- And more...

---

## 👤 Roles

- **Admin**: Full control, can pause/resume and withdraw protocol fees
- **Pauser**: Can pause/resume protocol
- **Fee Manager**: Can withdraw protocol fees
- **Governance**: Optional multisig for high-privilege actions
- **Creator**: Earns a portion of swap fees

---
