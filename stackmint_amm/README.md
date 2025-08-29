# StackMint AMM 🪙⚖️

## Overview

**StackMint AMM** is a robust, highly-configurable Automated Market Maker (AMM) protocol on Solana, built with the [Anchor framework](https://book.anchor-lang.com/). It enables seamless token swaps, liquidity provisioning, and fee routing, while offering rich features for protocol admins, pool creators, and liquidity providers. Designed with modularity and security in mind, StackMint AMM embraces best practices in DeFi composability, governance, and upgradability.

**NOTE** this project is a proof of concept/prototype that was developed in Solana Playground IDE


---

## ✨ Key Features

### 🏦 Core AMM Operations

- **Constant Product Market Maker (CPMM):** Traditional x*y=k invariant for swaps, ensuring liquidity at all price points.
- **Dual-Sided Liquidity Provision & Removal:** Deposit and withdraw tokens on both sides, receive LP tokens reflecting your share.
- **Canonical LP tokens:** LP tokens are minted and burned as canonical representations of pool ownership.

### 💸 Fee System

- **Multi-Tiered Fees:** Supports protocol-level, pool-level, and per-creator fee configurations.
- **Fee Routing:** Protocol and creator fees are separated and routed to dedicated vaults for accountability.
- **Configurable Caps:** Global and per-pool maximum fee caps for user safety.

### ⚙️ Pool Creation & Customization

- **Permissionless Pool Creation:** Anyone can create a pool for supported token pairs.
- **Decimal Normalization:** Pools can normalize tokens with different decimals for fair math.
- **Fee-on-Transfer Token Support:** Pools are compatible with tokens that deduct fees on transfer.
- **Treasury Dust Sweeping:** Small residuals ("dust") in vaults are automatically swept to the treasury.

### 🛡️ Security & Safety

- **Reentrancy Locks:** Pools are locked during state-changing operations.
- **Emergency Pause/Resume:** Admins/pausers can pause/resume all protocol activity.
- **Emergency Withdrawals:** LPs can withdraw their share even when the protocol is paused, ensuring user funds are always accessible.
- **Governance Approval Hooks:** Optional off-chain/multisig governance for sensitive actions, with replay protection.

### 📈 Oracle & Slippage Protection

- **Oracle Price Checks:** Integrates with oracles (Pyth will be added in future ) to ensure swaps stay within allowed deviation from market price.
- **User-Defined Slippage Checks:** All swap and mint/redeem operations enforce minimum output constraints.

### 🏆 Creator Incentives

- **Creator Fee Accrual:** Stack creators can earn fees from pool swaps.
- **Timelocked Fee Claiming:** Fees are claimable only after a configurable time lock, deterring abuse.

### 🔧 Admin & Governance Tools

- **Upgradeable Account Structures:** All major accounts are versioned and sized for future upgrades.
- **Configurable Protocol Parameters:** Admins can set protocol-wide and per-pool settings for parameters like fees, dust thresholds, claim locks, and price deviation caps.

---

## 🛠️ Program Architecture

### Modules & Entrypoints

The primary program is defined in [`src/lib.rs`](./src/lib.rs) and organized into:

- **Entrypoints:** Each major action (init, pool creation, swap, add/remove liquidity, fee claim, emergency ops, etc.) is a top-level function in the program module.
- **Contexts:** Each instruction has a context struct that specifies required accounts, access control, and constraints.
- **Structs:** On-chain state is managed with Anchor's `#[account]` structs, such as `Global`, `Pool`, `StackInfo`, and `GovernanceApproval`.
- **Helpers & Math:** Pure functions aid in normalization, fee calculation, and invariant math.
- **Events:** Anchor events are emitted for all major operations, facilitating off-chain indexing and transparency.
- **Error Codes:** All errors use descriptive variants (via Anchor’s `#[error_code]`), aiding debugging and UX.

---

## 🧩 State Structures

### **Global**
- Stores protocol-wide settings: admin/pauser/governance keys, protocol fee BPS, max fee cap, dust threshold, claim lock, treasury, and version.

### **StackInfo**
- Registered stack token metadata, including creator, mint, creator fee rate, optional rebalance hook, and bump seed.

### **Pool**
- Each pool contains:
  - Token mints (stack/quote), LP mint
  - Fee parameters, invariant constant `k`, bump
  - Vault addresses for tokens and fees
  - Total LP supply, decimal normalization, fee-on-transfer flag
  - Oracle account, price deviation cap
  - Creator claimable fees and last claim timestamp
  - Governance nonce for replay protection
  - Paused/locked flags

### **GovernanceApproval**
- Optional PDA account for off-chain/multisig governance.
- Contains target pool, approval flag, expiry timestamp, and strictly increasing nonce.

---

## 🔄 Main Functions (Instructions)

- **init_global:** Initialize protocol-wide state and admin roles.
- **register_stack:** Register a stack token and set creator/fee.
- **create_pool:** Set up a new AMM pool, including all vaults, fee accounts, and normalization.
- **provide_liquidity/remove_liquidity:** Add or withdraw liquidity to/from pools, mint/burn LP tokens, handle dust.
- **swap_stack_to_quote / swap_quote_to_stack:** Perform swaps with fee routing, oracle/slippage protection, and safety checks.
- **mint_stack_via_pool / redeem_stack_via_pool:** Mint new stack tokens or redeem for quote by swapping through the pool.
- **claim_creator_fees:** Claim accumulated creator fees after a configurable time lock.
- **emergency_pause / emergency_resume:** Pause or resume global protocol activity.
- **emergency_withdraw:** Allow LPs to withdraw funds even when paused.
- **withdraw_protocol_fees:** Admin/fee manager/governance can withdraw protocol fees from fee vaults.
- **view_mid_price:** Read-only helper to fetch the current pool price.
- **set_pool_params:** Update pool parameters like fee, k, and price deviation cap, optionally requiring governance approval.

---

## 📝 Events

For transparency and off-chain integration, all major actions emit events, including:

- `GlobalInitialized`
- `StackRegistered`
- `PoolCreated`
- `LiquidityProvided`
- `LiquidityRemoved`
- `Swap`
- `CreatorClaimed`
- `ProtocolFeesWithdrawn`
- `EmergencyWithdrawal`
- `MidPrice`
- `ProtocolPaused` / `ProtocolResumed`
- `PoolParamsUpdated`
- `MintedStackViaPool`
- `RedeemedStackViaPool`

These events are essential for indexers, explorers, and frontend UIs.

---

## ❗ Error Codes

All errors use descriptive Anchor error codes for clarity:

- `InvalidFee`
- `InvalidMintAuthority`
- `PoolPaused` / `ProtocolPaused`
- `MathOverflow`
- `SlippageExceeded`
- `Unauthorized`
- `NoLiquidity`
- `Reentrancy`
- `InvalidVaultOwner` / `InvalidVaultMint`
- `InvalidDecimals`
- `ZeroLpMint`
- `OraclePriceMismatch`
- `NoFees`
- `ClaimLocked`
- `GovernanceApprovalMissing` / `GovernanceNotApproved` / `GovernanceApprovalExpired` / `GovernanceApprovalTargetMismatch`

---

## 🧮 Key Variables & Constants

- `BPS_DENOM`: Basis points denominator (10,000)
- `DEFAULT_LP_DECIMALS`: Default LP token decimals
- `FALLBACK_DUST_THRESHOLD`: Default dust sweep threshold (10 units)
- `FALLBACK_CREATOR_CLAIM_LOCK_SECS`: Default creator claim lock (7 days)
- `FALLBACK_MAX_FEE_BPS`: Default max fee (20% cap)
- `INTERNAL_PRECISION_DECIMALS`: Normalization target (9 decimals)

These constants ensure sensible defaults and safety for all operations.

---

## 🔬 Math & Fee Logic

- **Normalization/Denormalization:** All amounts are internally normalized to a common precision for fair computation.
- **Fee Calculation:** Gross, protocol, and creator fees are centrally computed and routed.
- **Invariant Enforcement:** All swaps and liquidity actions are checked for invariant safety and overflow.
- **Oracle & Slippage:** Swaps can be protected with oracle price checks and user-defined slippage limits.

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

## 👤 Roles & Access Control

- **Admin:** Full protocol control (pause, withdraw, set params)
- **Pauser:** Can pause/resume protocol
- **Fee Manager:** Can withdraw protocol fees
- **Governance:** Optional multisig for sensitive actions
- **Creator:** Earns stack-specific swap fees, claimable after lock

---

## 🚀 Extensibility & Upgradeability

- All state structs are versioned and sized for future upgrades.
- Modular design allows new features, hooks, or fee types to be added safely.
- Governance hooks provide a pathway for decentralized upgrades and policy changes.

---

## NOTE
- The codebase demonstrates several practical strengths: most entrypoints include require! checks for pausing, fee bounds, reentrancy, and account validation, giving a broad set of defensive guards; protocol- and pool-level parameters can be updated by privileged roles or via governance approval, which enables operational flexibility; leftover “dust” tokens are swept to a treasury to avoid stuck tiny balances; fee routing is clearly separated so both protocol and creator fees are supported and auditable; Anchor PDAs are enforced for authority-sensitive accounts (vaults and mint authorities) to reduce the risk of unauthorized control; on-chain, replay-protected GovernanceApproval hooks exist for sensitive operations; and pools use a simple locked boolean to mitigate reentrancy. Taken together these design choices provide a solid starting point for safe experimentation and make the contract easier to reason about, audit, and extend.

- That said, this is intentionally a minimal proof-of-concept and should be treated as experimental: several areas remain incomplete or only lightly enforced. Oracle integration and optional rebalance hooks are currently TODOs or loosely validated, and the program relies on caller-supplied oracle prices rather than an independent oracle verification path; in a future iteration the Pyth network (or another secure feed) could be integrated to provide robust, on-chain price signals suitable for production. Fee accrual and claim logic exist but lack comprehensive test coverage, increasing the chance of subtle bugs; decimal normalization helps but can still incur rounding/precision edge cases for very low-liquidity pools or unusual token decimals; access controls are coarse and would benefit from more granular roles and auditing in a production rollout; there is no explicit upgradeability or migration strategy in this PoC; and overall the repository needs extensive adversarial and edge-case testing before any production use.

- This proof-of-concept already includes more advanced features than you’ll usually see in an initial prototype — I’ve iterated it through two separate versions so far — and those iterations have helped uncover useful patterns and gaps. I’m now debating whether to produce a third PoC (v3): a focused v3 could consolidate the improvements (fee helper refactors, Pyth or other oracle integration, hardened fixed-point math, expanded test suites, and explicit upgrade/migration paths), which would both make a future production rollout safer and provide a clear migration path for turning these ideas into a mainnet-grade project.

---


