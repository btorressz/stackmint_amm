use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, TokenAccount, Token, Transfer, MintTo, Burn};

declare_id!("7zcYfbAQNpGXpkfn5tXh7zMhJzm5UkQJeLbv2871cjVt");

const BPS_DENOM: u128 = 10_000u128;
const DEFAULT_LP_DECIMALS: u8 = 9;
const INTERNAL_PRECISION_DECIMALS: u8 = 9; // normalize to 9 decimals internally
const DUST_THRESHOLD: u64 = 10; // in token smallest units (adjust per token if desired)
const CREATOR_CLAIM_LOCK_SECS: i64 = 60 * 60 * 24 * 7; // 7 days timelock default

#[program]
pub mod stackmint_amm {
    use super::*;

    /// Initialize global state — now includes role pubkeys & versioning
    pub fn init_global(
        ctx: Context<InitGlobal>,
        protocol_fee_bps: u16,
        pauser: Pubkey,
        fee_manager: Pubkey,
        governance: Pubkey,
    ) -> Result<()> {
        let g = &mut ctx.accounts.global;
        require!(protocol_fee_bps <= 1000, AmmError::InvalidFee); // <=10%
        g.version = 1;
        g.admin = ctx.accounts.admin.key();
        g.pauser = pauser;
        g.fee_manager = fee_manager;
        g.governance = governance;
        g.protocol_fee_bps = protocol_fee_bps;
        g.paused = false;
        g.treasury = ctx.accounts.treasury.key();
        emit!(GlobalInitialized { admin: g.admin });
        Ok(())
    }

    /// Register a stack: provide stack_mint where program is the mint authority PDA
    pub fn register_stack(
        ctx: Context<RegisterStack>,
        creator_fee_bps: u16,
    ) -> Result<()> {
        let stack_info = &mut ctx.accounts.stack_info;
        require!(creator_fee_bps <= 5000, AmmError::InvalidFee); // arbitrary cap

        // Validate that provided stack_mint_auth is the expected PDA for this mint.
        // We avoid matching on the mint_authority field (its concrete type varies across spl-token versions).
        let (expected_pda, _bump_auth) = Pubkey::find_program_address(
            &[b"stack_mint_auth", ctx.accounts.stack_mint.key().as_ref()],
            &crate::ID,
        );
        require_keys_eq!(
            expected_pda,
            ctx.accounts.stack_mint_auth.key(),
            AmmError::InvalidMintAuthority
        );

        // compute bump for stack_info PDA and store it
        let (_expected_stack_info_pda, bump_stack_info) = Pubkey::find_program_address(
            &[b"stack_info", ctx.accounts.stack_mint.key().as_ref()],
            &crate::ID,
        );

        stack_info.version = 1;
        stack_info.creator = ctx.accounts.creator.key();
        stack_info.stack_mint = ctx.accounts.stack_mint.key();
        stack_info.creator_fee_bps = creator_fee_bps;
        stack_info.rebalance_hook = None;
        stack_info.bump = bump_stack_info;
        emit!(StackRegistered {
            stack_mint: stack_info.stack_mint,
            creator: stack_info.creator,
            creator_fee_bps,
        });
        Ok(())
    }

    /// Create pool. Requires the client to create vault token accounts whose owner is the vault_authority PDA.
    /// We assert vault ownership and mint match.
    #[allow(clippy::too_many_arguments)]
    pub fn create_pool(
        ctx: Context<CreatePool>,
        fee_bps: u16,
        k: u128,
        fee_on_transfer: bool,
        decimal_normalize_to: u8,
    ) -> Result<()> {
        // Basic checks
        require!(fee_bps <= 2000, AmmError::InvalidFee);
        require!(decimal_normalize_to <= 18, AmmError::InvalidDecimals);

        // Capture pool key BEFORE taking a mutable borrow to avoid borrow conflicts in emit! and other reads.
       let pool_key = ctx.accounts.pool.key();
       //commented out pool code might be needed for later(in order to improve tests quality)
       // let pool_key = ctx.accounts.pool.key();


        let pool = &mut ctx.accounts.pool;

      // let _pool_bump = ctx.accounts.pool.bump;
 // derive the vault_authority PDA and bump (server-side)
//let (expected_vault_auth, vault_auth_bump) = Pubkey::find_program_address(
   // &[b"vault_authority", pool_key.as_ref()],
  //  &crate::ID,
//);
//require_keys_eq!(expected_vault_auth, ctx.accounts.vault_authority.key(), AmmError::InvalidVaultOwner);



        // vault authority PDA must own both vault token accounts
        let vault_authority_key = ctx.accounts.vault_authority.key();
        require_keys_eq!(ctx.accounts.stack_vault.owner, vault_authority_key, AmmError::InvalidVaultOwner);
        require_keys_eq!(ctx.accounts.quote_vault.owner, vault_authority_key, AmmError::InvalidVaultOwner);

        // vault mints must match
        require_keys_eq!(ctx.accounts.stack_vault.mint, ctx.accounts.stack_mint.key(), AmmError::InvalidVaultMint);
        require_keys_eq!(ctx.accounts.quote_vault.mint, ctx.accounts.quote_mint.key(), AmmError::InvalidVaultMint);

        // fee vaults must be ATAs owned by vault authority and quote-mint denominated
        require_keys_eq!(ctx.accounts.protocol_fee_vault.owner, vault_authority_key, AmmError::InvalidVaultOwner);
        require_keys_eq!(ctx.accounts.creator_fee_vault.owner, vault_authority_key, AmmError::InvalidVaultOwner);
        require_keys_eq!(ctx.accounts.protocol_fee_vault.mint, ctx.accounts.quote_mint.key(), AmmError::InvalidVaultMint);
        require_keys_eq!(ctx.accounts.creator_fee_vault.mint, ctx.accounts.quote_mint.key(), AmmError::InvalidVaultMint);

        // Initialize pool
        pool.version = 1;
        pool.stack_mint = ctx.accounts.stack_mint.key();
        pool.quote_mint = ctx.accounts.quote_mint.key();
        pool.fee_bps = fee_bps;
        pool.k = k;
        pool.lp_mint = ctx.accounts.lp_mint.key();

        // compute bump for pool PDA and store it
        let (_expected_pool_pda, bump_pool) = Pubkey::find_program_address(
            &[b"pool", ctx.accounts.stack_mint.key().as_ref(), ctx.accounts.quote_mint.key().as_ref()],
            &crate::ID,
        );
        pool.bump = bump_pool;

        pool.paused = false;
        pool.total_lp_supply = 0u128;
        pool.locked = false;
        pool.decimal_normalize_to = decimal_normalize_to;
        pool.fee_on_transfer = fee_on_transfer;
        pool.protocol_fee_vault = ctx.accounts.protocol_fee_vault.key();
        pool.creator_fee_vault = ctx.accounts.creator_fee_vault.key();
        pool.treasury = ctx.accounts.global.treasury;
        pool.oracle = ctx.accounts.oracle.key();
        pool.creator_claimable = 0u128;
        pool.creator_last_claim_ts = 0i64;
        pool.max_price_deviation_bps = 2000; // default 20% allowed deviation vs provided oracle price
        emit!(PoolCreated {
            pool: pool_key,
            stack_mint: pool.stack_mint,
            quote_mint: pool.quote_mint,
            fee_bps,
        });
        Ok(())
    }

    /// Provide liquidity: normalized to internal precision, mint canonical LP shares using sqrt(total)
    /// Uses decimal-safe math and checks decimals.
    pub fn provide_liquidity(
        ctx: Context<ProvideLiquidity>,
        amount_stack: u64,
        amount_quote: u64,
    ) -> Result<()> {
        // Capture pool key & bump BEFORE taking a mutable borrow to avoid borrow conflicts
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        // Reentrancy & pause checks
        let pool = &mut ctx.accounts.pool;
        let global = &ctx.accounts.global;
        require!(!global.paused && !pool.paused, AmmError::ProtocolPaused);
        require!(!pool.locked, AmmError::Reentrancy);
        pool.locked = true;

        // decimal checks & normalization
        validate_token_account_matches_mint(&ctx.accounts.user_stack_account, &ctx.accounts.stack_mint)?;
        validate_token_account_matches_mint(&ctx.accounts.user_quote_account, &ctx.accounts.quote_mint)?;
        let stack_decimals = ctx.accounts.stack_mint.decimals;
        let quote_decimals = ctx.accounts.quote_mint.decimals;
        let target_decimals = pool.decimal_normalize_to;

        // For fee-on-transfer tokens, measure actual vault delta after transfer
        let reserve_stack_before = ctx.accounts.stack_vault.amount;
        let reserve_quote_before = ctx.accounts.quote_vault.amount;

        // Transfer tokens from user to vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_stack_account.to_account_info().clone(),
                    to: ctx.accounts.stack_vault.to_account_info().clone(),
                    authority: ctx.accounts.user.to_account_info().clone(),
                },
            ),
            amount_stack,
        )?;
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_quote_account.to_account_info().clone(),
                    to: ctx.accounts.quote_vault.to_account_info().clone(),
                    authority: ctx.accounts.user.to_account_info().clone(),
                },
            ),
            amount_quote,
        )?;

        // detect actual received amounts (handles fee-on-transfer)
        let reserve_stack_after = ctx.accounts.stack_vault.amount;
        let reserve_quote_after = ctx.accounts.quote_vault.amount;
        let actual_in_stack = reserve_stack_after.checked_sub(reserve_stack_before).ok_or(AmmError::MathOverflow)?;
        let actual_in_quote = reserve_quote_after.checked_sub(reserve_quote_before).ok_or(AmmError::MathOverflow)?;

        // Normalize amounts to common precision (u128)
        let norm_stack = normalize_amount_u128(actual_in_stack, stack_decimals, target_decimals)?;
        let norm_quote = normalize_amount_u128(actual_in_quote, quote_decimals, target_decimals)?;

        // Calculate LP to mint
        let lp_to_mint_u128: u128;
        if ctx.accounts.lp_mint.supply == 0 {
            // initial supply = floor(sqrt(norm_stack * norm_quote))
            let a = norm_stack;
            let b = norm_quote;
            lp_to_mint_u128 = integer_sqrt(a.checked_mul(b).ok_or(AmmError::MathOverflow)?);
        } else {
            // minted = norm_stack * total_lp / reserve_stack_norm_before
            let total_lp = ctx.accounts.lp_mint.supply as u128;
            // compute reserve_stack_norm based on vault reserves before transfer normalized
            let reserve_stack_norm_before = normalize_amount_u128(reserve_stack_before, stack_decimals, target_decimals)?;
            require!(reserve_stack_norm_before > 0, AmmError::NoLiquidity);
            lp_to_mint_u128 = norm_stack
                .checked_mul(total_lp).ok_or(AmmError::MathOverflow)?
                .checked_div(reserve_stack_norm_before).ok_or(AmmError::MathOverflow)?;
        }

        require!(lp_to_mint_u128 > 0, AmmError::ZeroLpMint);
        let lp_to_mint = lp_to_mint_u128.try_into().map_err(|_| AmmError::MathOverflow)?;

        // mint lp to user (vault_authority signs)
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.lp_mint.to_account_info().clone(),
                    to: ctx.accounts.user_lp_account.to_account_info().clone(),
                    authority: ctx.accounts.vault_authority.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
            lp_to_mint,
        )?;

        pool.total_lp_supply = pool.total_lp_supply.checked_add(lp_to_mint_u128).ok_or(AmmError::MathOverflow)?;

        // handle dust: if small leftover < DUST_THRESHOLD in vaults, sweep to treasury
        if ctx.accounts.stack_vault.amount <= DUST_THRESHOLD {
            let amt = ctx.accounts.stack_vault.amount;
            if amt > 0 {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.stack_vault.to_account_info().clone(),
                            to: ctx.accounts.treasury_token_account.to_account_info().clone(),
                            authority: ctx.accounts.vault_authority.to_account_info().clone(),
                        },
                    )
                    .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                    amt,
                )?;
            }
        }
        if ctx.accounts.quote_vault.amount <= DUST_THRESHOLD {
            let amt = ctx.accounts.quote_vault.amount;
            if amt > 0 {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.quote_vault.to_account_info().clone(),
                            to: ctx.accounts.treasury_token_account.to_account_info().clone(),
                            authority: ctx.accounts.vault_authority.to_account_info().clone(),
                        },
                    )
                    .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                    amt,
                )?;
            }
        }

        emit!(LiquidityProvided {
            pool: pool_key,
            provider: ctx.accounts.user.key(),
            lp_minted: lp_to_mint,
        });

        pool.locked = false;
        Ok(())
    }

    /// Remove liquidity — burn LP and withdraw pro rata in normalized units
    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, lp_amount: u64) -> Result<()> {
        // Capture pool key & bump BEFORE taking mutable borrow
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        let pool = &mut ctx.accounts.pool;
        let global = &ctx.accounts.global;
        require!(!global.paused && !pool.paused, AmmError::ProtocolPaused);
        require!(!pool.locked, AmmError::Reentrancy);
        pool.locked = true;

        let total_lp = ctx.accounts.lp_mint.supply as u128;
        require!(total_lp > 0, AmmError::NoLiquidity);

        // Burn LP from user
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.lp_mint.to_account_info().clone(),
                    from: ctx.accounts.user_lp_account.to_account_info().clone(),
                    authority: ctx.accounts.user.to_account_info().clone(),
                },
            ),
            lp_amount,
        )?;

        let lp_amount_u128 = lp_amount as u128;

        // compute normalized reserves
        let stack_decimals = ctx.accounts.stack_mint.decimals;
        let quote_decimals = ctx.accounts.quote_mint.decimals;
        let target_decimals = pool.decimal_normalize_to;

        let reserve_stack = ctx.accounts.stack_vault.amount;
        let reserve_quote = ctx.accounts.quote_vault.amount;

        let reserve_stack_norm = normalize_amount_u128(reserve_stack, stack_decimals, target_decimals)?;
        let reserve_quote_norm = normalize_amount_u128(reserve_quote, quote_decimals, target_decimals)?;

        // out_norm = reserve_norm * lp_amount / total_lp
        let out_stack_norm = reserve_stack_norm
            .checked_mul(lp_amount_u128).ok_or(AmmError::MathOverflow)?
            .checked_div(total_lp).ok_or(AmmError::MathOverflow)?;
        let out_quote_norm = reserve_quote_norm
            .checked_mul(lp_amount_u128).ok_or(AmmError::MathOverflow)?
            .checked_div(total_lp).ok_or(AmmError::MathOverflow)?;

        // denormalize back to native decimals
        let out_stack = denormalize_amount_u64(out_stack_norm, stack_decimals, target_decimals)?;
        let out_quote = denormalize_amount_u64(out_quote_norm, quote_decimals, target_decimals)?;

        // Transfer out tokens from vault to user (vault PDA signs)
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stack_vault.to_account_info().clone(),
                    to: ctx.accounts.user_stack_account.to_account_info().clone(),
                    authority: ctx.accounts.vault_authority.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
            out_stack,
        )?;
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.quote_vault.to_account_info().clone(),
                    to: ctx.accounts.user_quote_account.to_account_info().clone(),
                    authority: ctx.accounts.vault_authority.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
            out_quote,
        )?;

        // sweep dust if needed
        if ctx.accounts.stack_vault.amount <= DUST_THRESHOLD {
            let amt = ctx.accounts.stack_vault.amount;
            if amt > 0 {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.stack_vault.to_account_info().clone(),
                            to: ctx.accounts.treasury_token_account.to_account_info().clone(),
                            authority: ctx.accounts.vault_authority.to_account_info().clone(),
                        },
                    )
                    .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                    amt,
                )?;
            }
        }
        if ctx.accounts.quote_vault.amount <= DUST_THRESHOLD {
            let amt = ctx.accounts.quote_vault.amount;
            if amt > 0 {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.quote_vault.to_account_info().clone(),
                            to: ctx.accounts.treasury_token_account.to_account_info().clone(),
                            authority: ctx.accounts.vault_authority.to_account_info().clone(),
                        },
                    )
                    .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                    amt,
                )?;
            }
        }

        pool.total_lp_supply = pool.total_lp_supply.checked_sub(lp_amount_u128).ok_or(AmmError::MathOverflow)?;

        emit!(LiquidityRemoved {
            pool: pool_key,
            provider: ctx.accounts.user.key(),
            lp_burned: lp_amount,
        });

        pool.locked = false;
        Ok(())
    }

    /// Swap: stack -> quote. Dedicated context ensures proper accounts & reduces mistakes.
    pub fn swap_stack_to_quote(
        ctx: Context<SwapStackToQuote>,
        amount_in: u64,
        min_out: u64,
        oracle_price: Option<u128>, // optional oracle price in quote units per stack normalized to target decimals
        use_governance_approval: bool, // if true, require a GovernanceApproval PDA present and approved
    ) -> Result<()> {
        // capture pool key & bump before mutable borrow
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        let pool = &mut ctx.accounts.pool;
        let global = &ctx.accounts.global;

        // If governance approval requested, validate approval PDA
        if use_governance_approval {
            validate_governance_approval(&ctx.accounts.governance_approval, pool_key)?;
        }

        require!(!global.paused && !pool.paused, AmmError::ProtocolPaused);
        require!(!pool.locked, AmmError::Reentrancy);
        pool.locked = true;

        // validate decimals & accounts
        validate_token_account_matches_mint(&ctx.accounts.user_stack_account, &ctx.accounts.stack_mint)?;
        validate_token_account_matches_mint(&ctx.accounts.user_quote_account, &ctx.accounts.quote_mint)?;

        // Capture reserves before transfer
        let reserve_stack_before = ctx.accounts.stack_vault.amount;
        let reserve_quote_before = ctx.accounts.quote_vault.amount;

        // Transfer stack from user to vault (handles fee-on-transfer by measuring vault delta)
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_stack_account.to_account_info().clone(),
                    to: ctx.accounts.stack_vault.to_account_info().clone(),
                    authority: ctx.accounts.user.to_account_info().clone(),
                },
            ),
            amount_in,
        )?;

        let reserve_stack_after = ctx.accounts.stack_vault.amount;
        let actual_in_stack = reserve_stack_after.checked_sub(reserve_stack_before).ok_or(AmmError::MathOverflow)?;

        // normalize
        let stack_norm = normalize_amount_u128(actual_in_stack, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;
        let reserve_stack_norm = normalize_amount_u128(reserve_stack_before, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;
        let reserve_quote_norm = normalize_amount_u128(reserve_quote_before, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;

        // fees
        let gross_fee = stack_norm.checked_mul(pool.fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let protocol_fee = gross_fee.checked_mul(ctx.accounts.global.protocol_fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let creator_fee = gross_fee.checked_mul(ctx.accounts.stack_info.creator_fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let net_in = stack_norm.checked_sub(gross_fee).ok_or(AmmError::MathOverflow)?;

        // constant product out calculation in normalized units:
        let amount_out_norm = get_amount_out(net_in, reserve_stack_norm, reserve_quote_norm)?;

        // Price protection via provided oracle: check price deviation <= pool.max_price_deviation_bps
        if let Some(op) = oracle_price {
            if net_in == 0 {
                return Err(AmmError::SlippageExceeded.into());
            }
            let implied_price_x = amount_out_norm
                .checked_mul(10u128.pow(pool.decimal_normalize_to as u32)).ok_or(AmmError::MathOverflow)?
                .checked_div(net_in).ok_or(AmmError::MathOverflow)?;
            // Compare op and implied_price_x (both in same scaling expected from caller)
            let allowed = pool.max_price_deviation_bps as u128;
            let diff = if op > implied_price_x { op - implied_price_x } else { implied_price_x - op };
            let pct = diff.checked_mul(BPS_DENOM).ok_or(AmmError::MathOverflow)?.checked_div(op).ok_or(AmmError::MathOverflow)?;
            require!(pct <= allowed, AmmError::OraclePriceMismatch);
        }

        // convert amount_out_norm -> native quote units
        let amount_out = denormalize_amount_u64(amount_out_norm, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        require!(amount_out >= min_out, AmmError::SlippageExceeded);

        // Immediately route fees (protocol + creator) in normalized units -> convert to quote native units and transfer from vault to fee vaults
        let protocol_fee_quote_norm = get_amount_out(protocol_fee, reserve_stack_norm, reserve_quote_norm)?;
        let creator_fee_quote_norm = get_amount_out(creator_fee, reserve_stack_norm, reserve_quote_norm)?;

        let protocol_fee_quote_native = denormalize_amount_u64(protocol_fee_quote_norm, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        let creator_fee_quote_native = denormalize_amount_u64(creator_fee_quote_norm, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;

        // Transfer protocol fee from quote_vault -> protocol_fee_vault (vault PDA signs)
        if protocol_fee_quote_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.quote_vault.to_account_info().clone(),
                        to: ctx.accounts.protocol_fee_vault.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                protocol_fee_quote_native,
            )?;
        }
        if creator_fee_quote_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.quote_vault.to_account_info().clone(),
                        to: ctx.accounts.creator_fee_vault.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                creator_fee_quote_native,
            )?;
            // set claimable (normalized)
            pool.creator_claimable = pool.creator_claimable.checked_add(creator_fee_quote_norm).ok_or(AmmError::MathOverflow)?;
            pool.creator_last_claim_ts = Clock::get()?.unix_timestamp;
        }

        // Transfer amount_out from quote_vault to user (vault PDA signs)
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.quote_vault.to_account_info().clone(),
                    to: ctx.accounts.user_quote_account.to_account_info().clone(),
                    authority: ctx.accounts.vault_authority.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
            amount_out,
        )?;

        // sweep dust if tiny leftover
        if ctx.accounts.quote_vault.amount <= DUST_THRESHOLD {
            let amt = ctx.accounts.quote_vault.amount;
            if amt > 0 {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.quote_vault.to_account_info().clone(),
                            to: ctx.accounts.treasury_token_account.to_account_info().clone(),
                            authority: ctx.accounts.vault_authority.to_account_info().clone(),
                        },
                    )
                    .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                    amt,
                )?;
            }
        }

        emit!(Swap {
            pool: pool_key,
            trader: ctx.accounts.user.key(),
            side: SwapDirection::StackToQuote,
            amount_in,
            amount_out,
        });

        pool.locked = false;
        Ok(())
    }

    /// Swap: quote -> stack
    pub fn swap_quote_to_stack(
        ctx: Context<SwapQuoteToStack>,
        amount_in: u64,
        min_out: u64,
        oracle_price: Option<u128>,
        use_governance_approval: bool,
    ) -> Result<()> {
        // capture pool key & bump before mutable borrow
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        let pool = &mut ctx.accounts.pool;
        let global = &ctx.accounts.global;

        if use_governance_approval {
            validate_governance_approval(&ctx.accounts.governance_approval, pool_key)?;
        }

        require!(!global.paused && !pool.paused, AmmError::ProtocolPaused);
        require!(!pool.locked, AmmError::Reentrancy);
        pool.locked = true;

        // Validate accounts
        validate_token_account_matches_mint(&ctx.accounts.user_quote_account, &ctx.accounts.quote_mint)?;
        validate_token_account_matches_mint(&ctx.accounts.user_stack_account, &ctx.accounts.stack_mint)?;

        // reserves before
        let reserve_stack_before = ctx.accounts.stack_vault.amount;
        let reserve_quote_before = ctx.accounts.quote_vault.amount;

        // Transfer quote from user to quote_vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_quote_account.to_account_info().clone(),
                    to: ctx.accounts.quote_vault.to_account_info().clone(),
                    authority: ctx.accounts.user.to_account_info().clone(),
                },
            ),
            amount_in,
        )?;

        let reserve_quote_after = ctx.accounts.quote_vault.amount;
        let actual_in_quote = reserve_quote_after.checked_sub(reserve_quote_before).ok_or(AmmError::MathOverflow)?;

        // normalize
        let quote_norm = normalize_amount_u128(actual_in_quote, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        let reserve_quote_norm = normalize_amount_u128(reserve_quote_before, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        let reserve_stack_norm = normalize_amount_u128(reserve_stack_before, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;

        // compute fees (on quote input)
        let gross_fee = quote_norm.checked_mul(pool.fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let protocol_fee = gross_fee.checked_mul(ctx.accounts.global.protocol_fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let creator_fee = gross_fee.checked_mul(ctx.accounts.stack_info.creator_fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let net_in = quote_norm.checked_sub(gross_fee).ok_or(AmmError::MathOverflow)?;

        // constant product out calculation in normalized units:
        let amount_out_norm = get_amount_out(net_in, reserve_quote_norm, reserve_stack_norm)?;

        // price protection if oracle provided
        if let Some(op) = oracle_price {
            if net_in == 0 { return Err(AmmError::SlippageExceeded.into()); }
            let implied_price_x = amount_out_norm
                .checked_mul(10u128.pow(pool.decimal_normalize_to as u32)).ok_or(AmmError::MathOverflow)?
                .checked_div(net_in).ok_or(AmmError::MathOverflow)?;
            let allowed = pool.max_price_deviation_bps as u128;
            let diff = if op > implied_price_x { op - implied_price_x } else { implied_price_x - op };
            let pct = diff.checked_mul(BPS_DENOM).ok_or(AmmError::MathOverflow)?.checked_div(op).ok_or(AmmError::MathOverflow)?;
            require!(pct <= allowed, AmmError::OraclePriceMismatch);
        }

        // denormalize amount_out to native stack units
        let amount_out = denormalize_amount_u64(amount_out_norm, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;
        require!(amount_out >= min_out, AmmError::SlippageExceeded);

        // route fees: compute equivalent stack amount for protocol and creator fees (approx)
        let protocol_fee_stack_norm = get_amount_out(protocol_fee, reserve_quote_norm, reserve_stack_norm)?;
        let creator_fee_stack_norm = get_amount_out(creator_fee, reserve_quote_norm, reserve_stack_norm)?;
        let protocol_fee_stack_native = denormalize_amount_u64(protocol_fee_stack_norm, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;
        let creator_fee_stack_native = denormalize_amount_u64(creator_fee_stack_norm, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;

        if protocol_fee_stack_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.stack_vault.to_account_info().clone(),
                        to: ctx.accounts.protocol_fee_vault.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                protocol_fee_stack_native,
            )?;
        }
        if creator_fee_stack_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.stack_vault.to_account_info().clone(),
                        to: ctx.accounts.creator_fee_vault.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                creator_fee_stack_native,
            )?;
            pool.creator_claimable = pool.creator_claimable.checked_add(creator_fee_stack_norm).ok_or(AmmError::MathOverflow)?;
            pool.creator_last_claim_ts = Clock::get()?.unix_timestamp;
        }

        // transfer stacks to user
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stack_vault.to_account_info().clone(),
                    to: ctx.accounts.user_stack_account.to_account_info().clone(),
                    authority: ctx.accounts.vault_authority.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
            amount_out,
        )?;

        // sweep dust if tiny leftover
        if ctx.accounts.stack_vault.amount <= DUST_THRESHOLD {
            let amt = ctx.accounts.stack_vault.amount;
            if amt > 0 {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.stack_vault.to_account_info().clone(),
                            to: ctx.accounts.treasury_token_account.to_account_info().clone(),
                            authority: ctx.accounts.vault_authority.to_account_info().clone(),
                        },
                    )
                    .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                    amt,
                )?;
            }
        }

        emit!(Swap {
            pool: pool_key,
            trader: ctx.accounts.user.key(),
            side: SwapDirection::QuoteToStack,
            amount_in,
            amount_out,
        });

        pool.locked = false;
        Ok(())
    }

    /// Mint stack via pool: swap quote -> stack then mint stack tokens (stack mint authority PDA must sign)
    pub fn mint_stack_via_pool(
        ctx: Context<MintStackViaPool>,
        quote_in: u64,
        min_stack_out: u64,
        oracle_price: Option<u128>,
    ) -> Result<()> {
        // capture pool key & bump first
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        let pool = &mut ctx.accounts.pool;
        let global = &ctx.accounts.global;
        require!(!global.paused && !pool.paused, AmmError::ProtocolPaused);
        require!(!pool.locked, AmmError::Reentrancy);
        pool.locked = true;

        // Perform a quote->stack swap (reuse logic simplified)
        // reserves before
        let reserve_stack_before = ctx.accounts.stack_vault.amount;
        let reserve_quote_before = ctx.accounts.quote_vault.amount;

        // transfer quote in
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_quote_account.to_account_info().clone(),
                    to: ctx.accounts.quote_vault.to_account_info().clone(),
                    authority: ctx.accounts.user.to_account_info().clone(),
                },
            ),
            quote_in,
        )?;
        let reserve_quote_after = ctx.accounts.quote_vault.amount;
        let actual_in_quote = reserve_quote_after.checked_sub(reserve_quote_before).ok_or(AmmError::MathOverflow)?;

        // normalize
        let quote_norm = normalize_amount_u128(actual_in_quote, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        let reserve_quote_norm = normalize_amount_u128(reserve_quote_before, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        let reserve_stack_norm = normalize_amount_u128(reserve_stack_before, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;

        // fees
        let gross_fee = quote_norm.checked_mul(pool.fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let protocol_fee = gross_fee.checked_mul(ctx.accounts.global.protocol_fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let creator_fee = gross_fee.checked_mul(ctx.accounts.stack_info.creator_fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let net_in = quote_norm.checked_sub(gross_fee).ok_or(AmmError::MathOverflow)?;

        // compute amount_out normalized
        let amount_out_norm = get_amount_out(net_in, reserve_quote_norm, reserve_stack_norm)?;
        let amount_out_native = denormalize_amount_u64(amount_out_norm, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;
        require!(amount_out_native >= min_stack_out, AmmError::SlippageExceeded);

        // route fees to fee vaults (approximations as earlier)
        // compute protocol/creator fees in stack equivalent
        let protocol_fee_stack_norm = get_amount_out(protocol_fee, reserve_quote_norm, reserve_stack_norm)?;
        let creator_fee_stack_norm = get_amount_out(creator_fee, reserve_quote_norm, reserve_stack_norm)?;
        let protocol_fee_stack_native = denormalize_amount_u64(protocol_fee_stack_norm, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;
        let creator_fee_stack_native = denormalize_amount_u64(creator_fee_stack_norm, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;

        if protocol_fee_stack_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.stack_vault.to_account_info().clone(),
                        to: ctx.accounts.protocol_fee_vault.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                protocol_fee_stack_native,
            )?;
        }
        if creator_fee_stack_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.stack_vault.to_account_info().clone(),
                        to: ctx.accounts.creator_fee_vault.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                creator_fee_stack_native,
            )?;
            pool.creator_claimable = pool.creator_claimable.checked_add(creator_fee_stack_norm).ok_or(AmmError::MathOverflow)?;
            pool.creator_last_claim_ts = Clock::get()?.unix_timestamp;
        }

        // Mint stack tokens to user (stack_mint_auth PDA signs)
        let stack_info_bump = ctx.accounts.stack_info.bump;
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.stack_mint.to_account_info().clone(),
                    to: ctx.accounts.user_stack_account.to_account_info().clone(),
                    authority: ctx.accounts.stack_mint_auth.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"stack_mint_auth", ctx.accounts.stack_mint.to_account_info().key.as_ref(), &[stack_info_bump]]]),
            amount_out_native,
        )?;

        // sweep tiny dust from quote vault if needed
        if ctx.accounts.quote_vault.amount <= DUST_THRESHOLD {
            let amt = ctx.accounts.quote_vault.amount;
            if amt > 0 {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.quote_vault.to_account_info().clone(),
                            to: ctx.accounts.treasury_token_account.to_account_info().clone(),
                            authority: ctx.accounts.vault_authority.to_account_info().clone(),
                        },
                    )
                    .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                    amt,
                )?;
            }
        }

        emit!(MintedStackViaPool {
            pool: pool_key,
            user: ctx.accounts.user.key(),
            quote_in,
            stack_out: amount_out_native,
        });

        pool.locked = false;
        Ok(())
    }

    /// Redeem stack via pool: burn stacks and pay quote (approx via AMM)
    pub fn redeem_stack_via_pool(
        ctx: Context<RedeemStackViaPool>,
        stack_in: u64,
        min_quote_out: u64,
    ) -> Result<()> {
        // capture pool key & bump first
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        let pool = &mut ctx.accounts.pool;
        let global = &ctx.accounts.global;
        require!(!global.paused && !pool.paused, AmmError::ProtocolPaused);
        require!(!pool.locked, AmmError::Reentrancy);
        pool.locked = true;

        // Burn stack from user (we expect user transferred into their own account)
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.stack_mint.to_account_info().clone(),
                    from: ctx.accounts.user_stack_account.to_account_info().clone(),
                    authority: ctx.accounts.user.to_account_info().clone(),
                },
            ),
            stack_in,
        )?;

        // compute amounts using constant product
        let reserve_stack_before = ctx.accounts.stack_vault.amount;
        let reserve_quote_before = ctx.accounts.quote_vault.amount;
        let amount_in = stack_in as u128;

        // compute fees (on stack input)
        let gross_fee = amount_in
            .checked_mul(pool.fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let protocol_fee = gross_fee
            .checked_mul(ctx.accounts.global.protocol_fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let creator_fee = gross_fee
            .checked_mul(ctx.accounts.stack_info.creator_fee_bps as u128).ok_or(AmmError::MathOverflow)?
            .checked_div(BPS_DENOM).ok_or(AmmError::MathOverflow)?;
        let net_in = amount_in.checked_sub(gross_fee).ok_or(AmmError::MathOverflow)?;

        // normalize reserves
        let reserve_stack_norm = normalize_amount_u128(reserve_stack_before, ctx.accounts.stack_mint.decimals, pool.decimal_normalize_to)?;
        let reserve_quote_norm = normalize_amount_u128(reserve_quote_before, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;

        let amount_out_norm = get_amount_out(net_in, reserve_stack_norm, reserve_quote_norm)?;
        let amount_out_native = denormalize_amount_u64(amount_out_norm, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        require!(amount_out_native >= min_quote_out, AmmError::SlippageExceeded);

        // route fees: convert protocol & creator fee (denom: stack) to quote by simulation
        let protocol_fee_quote_norm = get_amount_out(protocol_fee, reserve_stack_norm, reserve_quote_norm)?;
        let creator_fee_quote_norm = get_amount_out(creator_fee, reserve_stack_norm, reserve_quote_norm)?;
        let protocol_fee_quote_native = denormalize_amount_u64(protocol_fee_quote_norm, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        let creator_fee_quote_native = denormalize_amount_u64(creator_fee_quote_norm, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;

        if protocol_fee_quote_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.quote_vault.to_account_info().clone(),
                        to: ctx.accounts.protocol_fee_vault.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                protocol_fee_quote_native,
            )?;
        }
        if creator_fee_quote_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.quote_vault.to_account_info().clone(),
                        to: ctx.accounts.creator_fee_vault.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                creator_fee_quote_native,
            )?;
            pool.creator_claimable = pool.creator_claimable.checked_add(creator_fee_quote_norm).ok_or(AmmError::MathOverflow)?;
            pool.creator_last_claim_ts = Clock::get()?.unix_timestamp;
        }

        // transfer quote_out to user
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.quote_vault.to_account_info().clone(),
                    to: ctx.accounts.user_quote_account.to_account_info().clone(),
                    authority: ctx.accounts.vault_authority.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
            amount_out_native,
        )?;

        emit!(RedeemedStackViaPool {
            pool: pool_key,
            user: ctx.accounts.user.key(),
            stack_in,
            quote_out: amount_out_native,
        });

        // sweep dust if tiny leftover
        if ctx.accounts.quote_vault.amount <= DUST_THRESHOLD {
            let amt = ctx.accounts.quote_vault.amount;
            if amt > 0 {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.quote_vault.to_account_info().clone(),
                            to: ctx.accounts.treasury_token_account.to_account_info().clone(),
                            authority: ctx.accounts.vault_authority.to_account_info().clone(),
                        },
                    )
                    .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                    amt,
                )?;
            }
        }

        pool.locked = false;
        Ok(())
    }

    /// Claim creator fees (timelocked)
    pub fn claim_creator_fees(ctx: Context<ClaimCreatorFees>) -> Result<()> {
        // capture pool key & bump before mutable borrow
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        let pool = &mut ctx.accounts.pool;
        let info = &ctx.accounts.stack_info;
        require!(ctx.accounts.creator.key() == info.creator, AmmError::Unauthorized);
        let now = Clock::get()?.unix_timestamp;
        require!(pool.creator_claimable > 0, AmmError::NoFees);
        require!(now >= pool.creator_last_claim_ts.checked_add(CREATOR_CLAIM_LOCK_SECS).ok_or(AmmError::MathOverflow)?, AmmError::ClaimLocked);

        // convert normalized claimable to native quote tokens
        let claim_norm = pool.creator_claimable;
        let amount_native = denormalize_amount_u64(claim_norm, ctx.accounts.quote_mint.decimals, pool.decimal_normalize_to)?;
        if amount_native > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.creator_fee_vault.to_account_info().clone(),
                        to: ctx.accounts.creator_receiver.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                amount_native,
            )?;
        }
        pool.creator_claimable = 0u128;
        emit!(CreatorClaimed {
            pool: pool_key,
            creator: ctx.accounts.creator.key(),
            amount: amount_native,
        });
        Ok(())
    }

    /// Pause/resume with pauser role
    pub fn emergency_pause(ctx: Context<PauseResume>) -> Result<()> {
        let g = &mut ctx.accounts.global;
        let caller = ctx.accounts.admin.key();
        require!(caller == g.admin || caller == g.pauser || caller == g.governance, AmmError::Unauthorized);
        g.paused = true;
        emit!(ProtocolPaused { by: caller });
        Ok(())
    }
    pub fn emergency_resume(ctx: Context<PauseResume>) -> Result<()> {
        let g = &mut ctx.accounts.global;
        let caller = ctx.accounts.admin.key();
        require!(caller == g.admin || caller == g.pauser || caller == g.governance, AmmError::Unauthorized);
        g.paused = false;
        emit!(ProtocolResumed { by: caller });
        Ok(())
    }

    /// Emergency withdraw (LPs can withdraw pro rata even if pool paused). For admin-only emergency drain, use different flow (not implemented here).
    pub fn emergency_withdraw(ctx: Context<EmergencyWithdraw>, lp_amount: u64) -> Result<()> {
        // capture pool key & bump first
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        // allow LP to burn and withdraw ignoring some checks, but still ensure math & non-negative
        let pool = &mut ctx.accounts.pool;
        require!(!pool.locked, AmmError::Reentrancy);
        pool.locked = true;

        // Burn LP & compute share
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.lp_mint.to_account_info().clone(),
                    from: ctx.accounts.user_lp_account.to_account_info().clone(),
                    authority: ctx.accounts.user.to_account_info().clone(),
                },
            ),
            lp_amount,
        )?;
        let lp_amount_u128 = lp_amount as u128;
        let total_lp = ctx.accounts.lp_mint.supply as u128;
        require!(total_lp > 0, AmmError::NoLiquidity);

        let out_stack = (ctx.accounts.stack_vault.amount as u128)
            .checked_mul(lp_amount_u128).ok_or(AmmError::MathOverflow)?
            .checked_div(total_lp).ok_or(AmmError::MathOverflow)?;
        let out_quote = (ctx.accounts.quote_vault.amount as u128)
            .checked_mul(lp_amount_u128).ok_or(AmmError::MathOverflow)?
            .checked_div(total_lp).ok_or(AmmError::MathOverflow)?;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stack_vault.to_account_info().clone(),
                    to: ctx.accounts.user_stack_account.to_account_info().clone(),
                    authority: ctx.accounts.vault_authority.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
            out_stack.try_into().map_err(|_| AmmError::MathOverflow)?,
        )?;
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.quote_vault.to_account_info().clone(),
                    to: ctx.accounts.user_quote_account.to_account_info().clone(),
                    authority: ctx.accounts.vault_authority.to_account_info().clone(),
                },
            )
            .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
            out_quote.try_into().map_err(|_| AmmError::MathOverflow)?,
        )?;

        pool.total_lp_supply = pool.total_lp_supply.checked_sub(lp_amount_u128).ok_or(AmmError::MathOverflow)?;

        pool.locked = false;
        emit!(EmergencyWithdrawal {
            pool: pool_key,
            user: ctx.accounts.user.key(),
            lp_burned: lp_amount,
        });
        Ok(())
    }

    /// Admin: withdraw accumulated protocol fees to admin receiver
    pub fn withdraw_protocol_fees(ctx: Context<WithdrawProtocolFees>, use_governance_approval: bool) -> Result<()> {
        // capture pool key & bump first
        let pool_key = ctx.accounts.pool.key();
        let pool_bump = ctx.accounts.pool.bump;

        let pool = &mut ctx.accounts.pool;
        let g = &ctx.accounts.global;
        let caller = ctx.accounts.admin.key();
        require!(caller == g.admin || caller == g.fee_manager || caller == g.governance, AmmError::Unauthorized);

        if use_governance_approval {
            validate_governance_approval(&ctx.accounts.governance_approval, pool_key)?;
        }

        // For simplicity: move all tokens in protocol_fee_vault to admin_receiver
        let vault_balance = ctx.accounts.protocol_fee_vault.amount;
        if vault_balance > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.protocol_fee_vault.to_account_info().clone(),
                        to: ctx.accounts.admin_receiver.to_account_info().clone(),
                        authority: ctx.accounts.vault_authority.to_account_info().clone(),
                    },
                )
                .with_signer(&[&[b"vault_authority", pool_key.as_ref(), &[pool_bump]]]),
                vault_balance,
            )?;
        }
        emit!(ProtocolFeesWithdrawn { pool: pool_key, to: ctx.accounts.admin_receiver.key(), amount: vault_balance });
        Ok(())
    }

    /// View helper (read-only): compute mid-price (quote per stack) from reserves — emits an event so off-chain indexers can read
    pub fn view_mid_price(ctx: Context<ViewMidPrice>) -> Result<()> {
        let pool_key = ctx.accounts.pool.key();

        let stack = ctx.accounts.stack_vault.amount as u128;
        let quote = ctx.accounts.quote_vault.amount as u128;
        require!(stack > 0 && quote > 0, AmmError::NoLiquidity);
        // price = quote / stack scaled to internal precision (10^decimal)
        let price_x = quote
            .checked_mul(10u128.pow(ctx.accounts.pool.decimal_normalize_to as u32)).ok_or(AmmError::MathOverflow)?
            .checked_div(stack).ok_or(AmmError::MathOverflow)?;
        emit!(MidPrice { pool: pool_key, price_x });
        Ok(())
    }

    /// Set pool parameters (admin/fee_manager/governance)
    pub fn set_pool_params(ctx: Context<SetParams>, new_fee_bps: Option<u16>, new_k: Option<u128>, max_price_deviation_bps: Option<u16>, use_governance_approval: bool) -> Result<()> {
        // capture pool key & bump first
        let pool_key = ctx.accounts.pool.key();

        let pool = &mut ctx.accounts.pool;
        let g = &ctx.accounts.global;
        let caller = ctx.accounts.admin.key();
        require!(caller == g.admin || caller == g.governance, AmmError::Unauthorized);

        if use_governance_approval {
            validate_governance_approval(&ctx.accounts.governance_approval, pool_key)?;
        }

        if let Some(f) = new_fee_bps {
            require!(f <= 5000, AmmError::InvalidFee);
            pool.fee_bps = f;
        }
        if let Some(kv) = new_k {
            pool.k = kv;
        }
        if let Some(m) = max_price_deviation_bps {
            pool.max_price_deviation_bps = m;
        }

        emit!(PoolParamsUpdated { pool: pool_key, by: caller });
        Ok(())
    }
}

/* ---------------------------------------------------
   ACCOUNTS, CONTEXTS, HELPERS, EVENTS & ERRORS
   --------------------------------------------------- */

#[derive(Accounts)]
pub struct InitGlobal<'info> {
    #[account(init, payer = admin, space = 8 + Global::LEN, seeds = [b"global"], bump)]
    pub global: Account<'info, Global>,
    #[account(mut)]
    pub admin: Signer<'info>,
    /// Treasury: ATA where dust and miscellaneous small balances are forwarded (denominated in quote mint for pools)
    #[account(mut)]
    pub treasury: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[account]
pub struct Global {
    pub version: u8,
    pub admin: Pubkey,
    pub pauser: Pubkey,
    pub fee_manager: Pubkey,
    pub governance: Pubkey,
    pub protocol_fee_bps: u16,
    pub paused: bool,
    pub treasury: Pubkey,
}
impl Global { const LEN: usize = 1 + 32*4 + 2 + 1 + 32; }

#[derive(Accounts)]
pub struct RegisterStack<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(mut)]
    pub stack_mint: Account<'info, Mint>,
    /// Mint authority PDA for the stack mint
    #[account(seeds = [b"stack_mint_auth", stack_mint.key().as_ref()], bump)]
    pub stack_mint_auth: UncheckedAccount<'info>,
    #[account(init, payer = creator, space = 8 + StackInfo::LEN, seeds=[b"stack_info", stack_mint.key().as_ref()], bump)]
    pub stack_info: Account<'info, StackInfo>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[account]
pub struct StackInfo {
    pub version: u8,
    pub creator: Pubkey,
    pub stack_mint: Pubkey,
    pub creator_fee_bps: u16,
    pub rebalance_hook: Option<Pubkey>,
    pub bump: u8,
}
impl StackInfo { const LEN: usize = 1 + 32 + 32 + 2 + (1+32) + 1; }

#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(mut)]
    pub stack_mint: Account<'info, Mint>,
    #[account(mut)]
    pub quote_mint: Account<'info, Mint>,

    /// LP mint
    #[account(mut)]
    pub lp_mint: Account<'info, Mint>,

    #[account(init, payer = creator, space = 8 + Pool::LEN, seeds=[b"pool", stack_mint.key().as_ref(), quote_mint.key().as_ref()], bump)]
    pub pool: Account<'info, Pool>,

    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,

    /// Fee vaults (ATAs owned by vault_authority)
    #[account(mut)]
    pub protocol_fee_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub creator_fee_vault: Account<'info, TokenAccount>,

    /// vault authority PDA
    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    /// optional oracle account (unchecked; integration with Pyth left as NOTE)
    /// client may pass real Pyth account here; program currently only stores pubkey
    #[account(mut)]
    pub oracle: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,

    #[account(seeds=[b"global"], bump)]
    pub global: Account<'info, Global>,
}

#[account]
pub struct Pool {
    pub version: u8,
    pub stack_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub fee_bps: u16,
    pub k: u128,
    pub bump: u8,
    pub paused: bool,
    pub total_lp_supply: u128,
    pub locked: bool,
    pub decimal_normalize_to: u8,
    pub fee_on_transfer: bool,
    pub protocol_fee_vault: Pubkey,
    pub creator_fee_vault: Pubkey,
    pub treasury: Pubkey,
    pub oracle: Pubkey,
    pub creator_claimable: u128, // normalized units
    pub creator_last_claim_ts: i64,
    pub max_price_deviation_bps: u16,
}
impl Pool {
    // rough size calc; adjust if you expand fields
    const LEN: usize = 1 + 32*6 + 2 + 16 + 1 + 1 + 1 + 4 + 32 + 32 + 32 + 16 + 8 + 2;
}

/* PROVIDE LIQUIDITY CONTEXT */
#[derive(Accounts)]
pub struct ProvideLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub stack_mint: Account<'info, Mint>,
    #[account(mut)]
    pub quote_mint: Account<'info, Mint>,

    /// vault token accounts (owned by vault_authority PDA)
    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,

    /// fee vaults
    #[account(mut)]
    pub protocol_fee_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub creator_fee_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub lp_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_lp_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_stack_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_quote_account: Account<'info, TokenAccount>,

    /// treasury ATA (quote-mint) where small dust amounts are swept
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,

    /// vault authority PDA
    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub global: Account<'info, Global>,
}

/* REMOVE LIQUIDITY CONTEXT */
#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(mut)]
    pub lp_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_lp_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,

    // add mint accounts so we can read decimals
    #[account(mut)]
    pub stack_mint: Account<'info, Mint>,
    #[account(mut)]
    pub quote_mint: Account<'info, Mint>,

    #[account(mut)]
    pub user_stack_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_quote_account: Account<'info, TokenAccount>,

    /// treasury ATA for dust
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,

    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub global: Account<'info, Global>,
}

/* SWAP CONTEXTS — separated directions for safety */

/* SWAP Stack -> Quote */
#[derive(Accounts)]
pub struct SwapStackToQuote<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(mut)]
    pub stack_mint: Account<'info, Mint>,
    #[account(mut)]
    pub quote_mint: Account<'info, Mint>,

    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_stack_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_quote_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub protocol_fee_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub creator_fee_vault: Account<'info, TokenAccount>,

    /// treasury ATA for dust sweeps
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,

    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(mut, seeds=[b"stack_info", stack_mint.key().as_ref()], bump)]
    pub stack_info: Account<'info, StackInfo>,

    /// optional governance approval PDA created by off-chain multisig flows
    pub governance_approval: Option<Account<'info, GovernanceApproval>>,

    pub token_program: Program<'info, Token>,
    pub global: Account<'info, Global>,
}

/* SWAP Quote -> Stack */
#[derive(Accounts)]
pub struct SwapQuoteToStack<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub pool: Account<'info, Pool>,

    #[account(mut)]
    pub stack_mint: Account<'info, Mint>,
    #[account(mut)]
    pub quote_mint: Account<'info, Mint>,

    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_stack_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_quote_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub protocol_fee_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub creator_fee_vault: Account<'info, TokenAccount>,

    /// treasury ATA for dust sweeps
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,

    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(mut, seeds=[b"stack_info", stack_mint.key().as_ref()], bump)]
    pub stack_info: Account<'info, StackInfo>,

    /// optional governance approval PDA
    pub governance_approval: Option<Account<'info, GovernanceApproval>>,

    pub token_program: Program<'info, Token>,
    pub global: Account<'info, Global>,
}

/* MintStackViaPool & RedeemStackViaPool contexts — include treasuries & fee vaults */
#[derive(Accounts)]
pub struct MintStackViaPool<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub stack_mint: Account<'info, Mint>,
    #[account(mut)]
    pub quote_mint: Account<'info, Mint>,
    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_quote_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_stack_account: Account<'info, TokenAccount>,
    #[account(seeds=[b"stack_mint_auth", stack_mint.key().as_ref()], bump)]
    pub stack_mint_auth: UncheckedAccount<'info>,
    #[account(mut)]
    pub protocol_fee_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub creator_fee_vault: Account<'info, TokenAccount>,
    #[account(mut, seeds=[b"stack_info", stack_mint.key().as_ref()], bump)]
    pub stack_info: Account<'info, StackInfo>,

    /// treasury ATA for dust sweeps
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,

    /// vault authority PDA
    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub global: Account<'info, Global>,
}

#[derive(Accounts)]
pub struct RedeemStackViaPool<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub stack_mint: Account<'info, Mint>,
    #[account(mut)]
    pub quote_mint: Account<'info, Mint>,
    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_stack_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_quote_account: Account<'info, TokenAccount>,
    #[account(mut, seeds=[b"stack_info", stack_mint.key().as_ref()], bump)]
    pub stack_info: Account<'info, StackInfo>,
    #[account(mut)]
    pub protocol_fee_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub creator_fee_vault: Account<'info, TokenAccount>,

    /// treasury ATA
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,

    /// vault authority PDA (needed by transfers)
    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub global: Account<'info, Global>,
}

/* Claim Creator Fees */
#[derive(Accounts)]
pub struct ClaimCreatorFees<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub stack_info: Account<'info, StackInfo>,
    #[account(mut)]
    pub quote_mint: Account<'info, Mint>,
    #[account(mut)]
    pub creator_fee_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub creator_receiver: Account<'info, TokenAccount>,
    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

/* Pause/Resume context used for both emergency_pause and resume */
#[derive(Accounts)]
pub struct PauseResume<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, seeds=[b"global"], bump)]
    pub global: Account<'info, Global>,
}

/* Emergency withdraw context uses same accounts as RemoveLiquidity but allows even when paused */
#[derive(Accounts)]
pub struct EmergencyWithdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub lp_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_lp_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_stack_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_quote_account: Account<'info, TokenAccount>,
    #[account(seeds=[b"vault_authority", pool.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

/* Withdraw protocol fees by admin/fee_manager/governance */
#[derive(Accounts)]
pub struct WithdrawProtocolFees<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub protocol_fee_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub admin_receiver: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_authority: UncheckedAccount<'info>,
    /// optional governance approval PDA
    pub governance_approval: Option<Account<'info, GovernanceApproval>>,
    pub token_program: Program<'info, Token>,
    #[account(seeds=[b"global"], bump)]
    pub global: Account<'info, Global>,
}

/* View mid price context */
#[derive(Accounts)]
pub struct ViewMidPrice<'info> {
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub stack_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub quote_vault: Account<'info, TokenAccount>,
}

/* SetParams context for set_pool_params instruction */
#[derive(Accounts)]
pub struct SetParams<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(seeds=[b"global"], bump)]
    pub global: Account<'info, Global>,
    /// optional governance approval PDA
    pub governance_approval: Option<Account<'info, GovernanceApproval>>,
}

/* -----------------------
   GovernanceApproval PDA (integration point for multisig)
   External multisig programs can create this PDA and set `approved = true`.
   The program treats this as a valid attestation for sensitive admin operations.
   ----------------------- */

#[account]
pub struct GovernanceApproval {
    /// which pool this approval is for (or 0 if global)
    pub target: Pubkey,
    /// true if multisig has approved this action
    pub approved: bool,
    /// expiry to prevent reuse
    pub expiry_ts: i64,
}
impl GovernanceApproval { const LEN: usize = 32 + 1 + 8; }

/* -----------------------
   EVENTS
   ----------------------- */

#[event]
pub struct GlobalInitialized { pub admin: Pubkey }

#[event]
pub struct StackRegistered {
    pub stack_mint: Pubkey,
    pub creator: Pubkey,
    pub creator_fee_bps: u16,
}

#[event]
pub struct PoolCreated {
    pub pool: Pubkey,
    pub stack_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub fee_bps: u16,
}

#[event]
pub struct LiquidityProvided {
    pub pool: Pubkey,
    pub provider: Pubkey,
    pub lp_minted: u64,
}

#[event]
pub struct LiquidityRemoved {
    pub pool: Pubkey,
    pub provider: Pubkey,
    pub lp_burned: u64,
}

#[event]
pub struct Swap {
    pub pool: Pubkey,
    pub trader: Pubkey,
    pub side: SwapDirection,
    pub amount_in: u64,
    pub amount_out: u64,
}

#[event]
pub struct CreatorClaimed {
    pub pool: Pubkey,
    pub creator: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ProtocolFeesWithdrawn {
    pub pool: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
}

#[event]
pub struct EmergencyWithdrawal {
    pub pool: Pubkey,
    pub user: Pubkey,
    pub lp_burned: u64,
}

#[event]
pub struct MidPrice {
    pub pool: Pubkey,
    pub price_x: u128,
}

#[event]
pub struct ProtocolPaused { pub by: Pubkey }
#[event]
pub struct ProtocolResumed { pub by: Pubkey }
#[event]
pub struct PoolParamsUpdated { pub pool: Pubkey, pub by: Pubkey }

/* extra events for minted/redeemed flows */
#[event]
pub struct MintedStackViaPool { pub pool: Pubkey, pub user: Pubkey, pub quote_in: u64, pub stack_out: u64 }
#[event]
pub struct RedeemedStackViaPool { pub pool: Pubkey, pub user: Pubkey, pub stack_in: u64, pub quote_out: u64 }
#[event]
pub struct ProtocolFeesWithdrawn2 { pub pool: Pubkey, pub to: Pubkey, pub amount: u64 }

/* -----------------------
   ENUMS & HELPERS
   ----------------------- */

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum SwapDirection { StackToQuote, QuoteToStack }

fn integer_sqrt(value: u128) -> u128 {
    if value <= 1 { return value; }
    let mut left: u128 = 1;
    let mut right: u128 = value;
    while left <= right {
        let mid = (left + right) >> 1;
        let sq = mid.saturating_mul(mid);
        if sq == value { return mid; }
        if sq < value { left = mid + 1; } else { right = mid - 1; }
    }
    right
}

/// central math wrapper example
fn checked_mul_div(a: u128, b: u128, c: u128) -> Result<u128> {
    // convert AmmError into anchor error explicitly to avoid type inference issues
    let mul = a.checked_mul(b).ok_or(AmmError::MathOverflow)?;
    let div = mul.checked_div(c).ok_or(AmmError::MathOverflow)?;
    Ok(div)
}

/// Normalize any token amount to `target_decimals` as u128 (for internal math)
fn normalize_amount_u128(amount: u64, src_decimals: u8, target_decimals: u8) -> Result<u128> {
    if src_decimals == target_decimals {
        return Ok(amount as u128);
    } else if src_decimals < target_decimals {
        let mul = 10u128.pow((target_decimals - src_decimals) as u32);
        Ok((amount as u128).checked_mul(mul).ok_or(AmmError::MathOverflow)?)
    } else {
        let div = 10u128.pow((src_decimals - target_decimals) as u32);
        Ok((amount as u128).checked_div(div).ok_or(AmmError::MathOverflow)?)
    }
}

/// Denormalize from normalized u128 back to native decimals u64 (floor)
fn denormalize_amount_u64(amount_norm: u128, dst_decimals: u8, target_decimals: u8) -> Result<u64> {
    if dst_decimals == target_decimals {
        Ok(amount_norm.try_into().map_err(|_| AmmError::MathOverflow)?)
    } else if dst_decimals < target_decimals {
        let div = 10u128.pow((target_decimals - dst_decimals) as u32);
        Ok(amount_norm.checked_div(div).ok_or(AmmError::MathOverflow)?.try_into().map_err(|_| AmmError::MathOverflow)?)
    } else {
        let mul = 10u128.pow((dst_decimals - target_decimals) as u32);
        Ok(amount_norm.checked_mul(mul).ok_or(AmmError::MathOverflow)?.try_into().map_err(|_| AmmError::MathOverflow)?)
    }
}

/// Validate that token account matches the mint provided
fn validate_token_account_matches_mint(token_acc: &Account<TokenAccount>, mint: &Account<Mint>) -> Result<()> {
    require_keys_eq!(token_acc.mint, mint.key(), AmmError::InvalidVaultMint);
    Ok(())
}

/// constant-product get amount out (u128 arithmetic)
fn get_amount_out(amount_in: u128, reserve_in: u128, reserve_out: u128) -> Result<u128> {
    // x*y=k: out = amount_in * reserve_out / (reserve_in + amount_in)
    require!(reserve_in > 0 && reserve_out > 0, AmmError::NoLiquidity);
    let numerator = amount_in.checked_mul(reserve_out).ok_or(AmmError::MathOverflow)?;
    let denominator = reserve_in.checked_add(amount_in).ok_or(AmmError::MathOverflow)?;
    let out = numerator.checked_div(denominator).ok_or(AmmError::MathOverflow)?;
    Ok(out)
}

/// Validate GovernanceApproval PDA (simple integration point for external multisig)
fn validate_governance_approval(maybe_approval: &Option<Account<GovernanceApproval>>, target: Pubkey) -> Result<()> {
    match maybe_approval {
        Some(approval) => {
            require!(approval.approved, AmmError::GovernanceNotApproved);
            require!(approval.expiry_ts >= Clock::get()?.unix_timestamp, AmmError::GovernanceApprovalExpired);
            require_keys_eq!(approval.target, target, AmmError::GovernanceApprovalTargetMismatch);
            Ok(())
        }
        None => Err(AmmError::GovernanceApprovalMissing.into()),
    }
}

/* -----------------------
   Errors
   ----------------------- */

#[error_code]
pub enum AmmError {
    #[msg("Invalid fee")]
    InvalidFee,
    #[msg("Invalid mint authority")]
    InvalidMintAuthority,
    #[msg("Pool is paused")]
    PoolPaused,
    #[msg("Protocol is paused")]
    ProtocolPaused,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("No liquidity")]
    NoLiquidity,
    #[msg("Reentrancy detected")]
    Reentrancy,
    #[msg("Invalid vault owner")]
    InvalidVaultOwner,
    #[msg("Invalid vault mint")]
    InvalidVaultMint,
    #[msg("Invalid decimals")]
    InvalidDecimals,
    #[msg("Zero LP minted")]
    ZeroLpMint,
    #[msg("Oracle price mismatch")]
    OraclePriceMismatch,
    #[msg("No fees to claim")]
    NoFees,
    #[msg("Creator claim locked")]
    ClaimLocked,
    #[msg("Governance approval missing")]
    GovernanceApprovalMissing,
    #[msg("Governance not approved")]
    GovernanceNotApproved,
    #[msg("Governance approval expired")]
    GovernanceApprovalExpired,
    #[msg("Governance approval target mismatch")]
    GovernanceApprovalTargetMismatch,
}
