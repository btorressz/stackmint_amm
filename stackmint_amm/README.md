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

### NOTE 

- The codebase demonstrates several practical strengths: most entrypoints include require! checks for pausing, fee bounds, reentrancy, and account validation, giving a broad set of defensive guards; protocol- and pool-level parameters can be updated by privileged roles or via governance approval, which enables operational flexibility; leftover “dust” tokens are swept to a treasury to avoid stuck tiny balances; fee routing is clearly separated so both protocol and creator fees are supported and auditable; Anchor PDAs are enforced for authority-sensitive accounts (vaults and mint authorities) to reduce the risk of unauthorized control; on-chain, replay-protected GovernanceApproval hooks exist for sensitive operations; and pools use a simple locked boolean to mitigate reentrancy. Taken together these design choices provide a solid starting point for safe experimentation and make the contract easier to reason about, audit, and extend.

- That said, this is intentionally a minimal proof-of-concept and should be treated as experimental: several areas remain incomplete or only lightly enforced. Oracle integration and optional rebalance hooks are currently TODOs or loosely validated, and the program relies on caller-supplied oracle prices rather than an independent oracle verification path; in a future iteration the Pyth network (or another secure feed) could be integrated to provide robust, on-chain price signals suitable for production. Fee accrual and claim logic exist but lack comprehensive test coverage, increasing the chance of subtle bugs; decimal normalization helps but can still incur rounding/precision edge cases for very low-liquidity pools or unusual token decimals; access controls are coarse and would benefit from more granular roles and auditing in a production rollout; there is no explicit upgradeability or migration strategy in this PoC; and overall the repository needs extensive adversarial and edge-case testing before any production use.

- This proof-of-concept already includes more advanced features than you’ll usually see in an initial prototype — I’ve iterated it through two separate versions so far — and those iterations have helped uncover useful patterns and gaps. I’m now debating whether to produce a third PoC (v3): a focused v3 could consolidate the improvements (fee helper refactors, Pyth or other oracle integration, hardened fixed-point math, expanded test suites, and explicit upgrade/migration paths), which would both make a future production rollout safer and provide a clear migration path for turning these ideas into a mainnet-grade project. 
