// tests/stackmint_amm.diagnostics.test.ts(anchor.test.ts in solana playground)
//edit tests 
import assert from "assert";
import * as anchor from "@project-serum/anchor";
import {
  Keypair,
  Transaction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  PublicKey,
  LAMPORTS_PER_SOL,
  sendAndConfirmTransaction,
  Connection,
} from "@solana/web3.js";
import * as splToken from "@solana/spl-token";

const BN = (anchor as any).BN ?? (anchor as any).bn ?? ((v: any) => v);

const PROGRAM_ID = new PublicKey("7zcYfbAQNpGXpkfn5tXh7zMhJzm5UkQJeLbv2871cjVt");

describe("stackmint_amm diagnostics test", () => {
  it("runs the main flows with extensive diagnostics", async function () {
    // allow long-running tests (Anchor/playground + airdrops)
    this.timeout?.(180000);

    // ---------- Resolve provider / connection / program ----------
    let provider: any = undefined;
    let connection: Connection | any = undefined;
    let program: any = undefined; // <-- typed `any` to avoid 'unknown' property errors

    // If Playground (pg) is present, prefer it.
    try {
      // eslint-disable-next-line @typescript-eslint/ban-ts-comment
      // @ts-ignore
      if (typeof pg !== "undefined" && pg) {
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
        if (pg.provider) {
          // eslint-disable-next-line @typescript-eslint/ban-ts-comment
          // @ts-ignore
          provider = pg.provider;
          // eslint-disable-next-line @typescript-eslint/ban-ts-comment
          // @ts-ignore
          connection = pg.connection;
          // eslint-disable-next-line @typescript-eslint/ban-ts-comment
          // @ts-ignore
          program = pg.program;
          console.log("Using Playground-provided provider/program.");
        } else if (pg.program && pg.connection) {
          // eslint-disable-next-line @typescript-eslint/ban-ts-comment
          // @ts-ignore
          program = pg.program;
          // eslint-disable-next-line @typescript-eslint/ban-ts-comment
          // @ts-ignore
          connection = pg.connection;
          // create a local provider wrapper
          const AnchorProviderRuntime = (anchor as any).AnchorProvider ?? (anchor as any).Provider;
          provider = AnchorProviderRuntime?.local
            ? AnchorProviderRuntime.local(connection)
            : (anchor as any).getProvider?.() ?? AnchorProviderRuntime?.env?.() ?? null;
          if (provider) anchor.setProvider(provider);
          console.log("Using Playground program & connection fallback to local provider.");
        }
      }
    } catch (e) {
      // ignore; we'll fallback below
    }

    // Fallback to AnchorProvider.env() or anchor.getProvider()
    if (!provider) {
      try {
        // Use runtime lookup to avoid TypeScript export/typing issues
        const AnchorProviderRuntime = (anchor as any).AnchorProvider ?? (anchor as any).Provider ?? undefined;
        if (AnchorProviderRuntime && typeof AnchorProviderRuntime.env === "function") {
          provider = AnchorProviderRuntime.env();
        } else if ((anchor as any).getProvider) {
          provider = (anchor as any).getProvider();
        } else {
          // last resort, try Provider.local()
          const localConn = new anchor.web3.Connection("http://127.0.0.1:8899", "confirmed");
          provider = (anchor as any).Provider?.local
            ? (anchor as any).Provider.local(localConn)
            : (anchor as any).AnchorProvider?.local(localConn);
        }
        if (!provider) throw new Error("Failed to derive provider via available Anchor runtime helpers.");
        anchor.setProvider(provider);
        connection = provider.connection;
        console.log("Using Anchor provider derived at runtime.");
      } catch (e) {
        throw new Error("Unable to derive Anchor provider. Ensure tests run with Anchor environment or Playground. Error: " + String(e));
      }
    }

    // If program not set, try anchor.workspace lookup
    if (!program) {
      try {
        const workspace = (anchor as any).workspace ?? {};
        const workspaceEntries = Object.entries(workspace);
        if (workspaceEntries.length > 0) {
          for (const [k, p] of workspaceEntries) {
            try {
              if ((p as any).programId && (p as any).programId.equals?.(PROGRAM_ID)) {
                program = p;
                console.log("Found program in anchor.workspace by programId:", k);
                break;
              }
            } catch {
              // ignore
            }
          }
          if (!program) {
            program = workspaceEntries[0][1];
            console.log("Using first program from anchor.workspace:", workspaceEntries[0][0]);
          }
        }
      } catch (e) {
        // ignore - we'll try constructing from IDL below
      }
    }

    // If still not found, attempt to fetch IDL on-chain and construct program
    if (!program) {
      try {
        const idl = await (anchor as any).Program.fetchIdl(PROGRAM_ID, provider);
        if (!idl) throw new Error("No IDL found on-chain and anchor.workspace empty.");
        program = new (anchor as any).Program(idl, PROGRAM_ID, provider);
        console.log("Constructed program from on-chain IDL.");
      } catch (err) {
        throw new Error("Unable to find or construct anchor Program. Ensure program deployed and IDL available. Err: " + String(err));
      }
    }

    if (!provider || !connection || !program) {
      throw new Error("Provider/Connection/Program resolution failure. Aborting test.");
    }

    // ensure program is `any` and compute a deterministic program id for PDA derivation
    const progId: PublicKey = (program as any).programId ?? PROGRAM_ID;
    console.log("Resolved program id (progId):", progId.toBase58());

    // create a local payer for utility operations (ATA creation / mints)
    const payer = Keypair.generate();
    console.log("Local helper payer:", payer.publicKey.toBase58());

    try {
      const sig = await connection.requestAirdrop(payer.publicKey, LAMPORTS_PER_SOL);
      await connection.confirmTransaction(sig, "confirmed");
      console.log("Airdropped to helper payer:", sig);
    } catch (e) {
      console.warn("Airdrop to helper payer failed or not needed in this environment:", e);
    }

    const adminPubkey: PublicKey = provider.wallet.publicKey;
    console.log("Resolved admin/test wallet pubkey:", adminPubkey.toBase58());
    console.log("Program ID:", progId.toBase58());

    // helpers and constants
    const TOKEN_PROGRAM_ID = (splToken as any).TOKEN_PROGRAM_ID ?? (splToken as any).TOKEN_PROGRAM_ID;
    const ASSOCIATED_TOKEN_PROGRAM_ID =
      (splToken as any).ASSOCIATED_TOKEN_PROGRAM_ID ?? (splToken as any).ASSOCIATED_TOKEN_PROGRAM_ID;

    async function printTxLogs(sig: string | null | undefined) {
      if (!sig) {
        console.warn("No signature to show logs for.");
        return;
      }
      try {
        const tx = await connection.getTransaction(sig, { commitment: "confirmed" });
        if (!tx) {
          console.warn("Transaction not found for sig:", sig);
          return;
        }
        console.log(`=== logs for ${sig} ===`);
        (tx.meta?.logMessages ?? []).forEach((l: string) => console.log("   ", l));
        console.log("=== end logs ===");
      } catch (e) {
        console.warn("Error fetching tx logs:", e);
      }
    }

    async function ensureExists(pubkey: PublicKey, label: string) {
      const info = await connection.getAccountInfo(pubkey);
      if (!info) throw new Error(`Missing required account: ${label} (${pubkey.toBase58()})`);
      console.log(`OK: ${label} exists: ${pubkey.toBase58()}`);
    }

    // spl-token helpers (use any to avoid version typing issues)
    async function createMint(decimals: number, mintAuthority: PublicKey): Promise<PublicKey> {
      // many spl-token versions have different signatures; we use the compatibility wrapper in `any`
      const mint = await (splToken as any).createMint(connection, payer, mintAuthority, null, decimals);
      console.log(`Created mint ${mint.toBase58()} decimals=${decimals} authority=${mintAuthority.toBase58()}`);
      return mint;
    }

    async function getOrCreateAtaAndLog(owner: PublicKey, mint: PublicKey, label: string): Promise<PublicKey> {
      const ata = await (splToken as any).getAssociatedTokenAddress(mint, owner, false, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);
      const info = await connection.getAccountInfo(ata);
      if (!info) {
        const ix = (splToken as any).createAssociatedTokenAccountInstruction(
          payer.publicKey,
          ata,
          owner,
          mint,
          TOKEN_PROGRAM_ID,
          ASSOCIATED_TOKEN_PROGRAM_ID
        );
        const tx = new Transaction().add(ix);
        const sig = await sendAndConfirmTransaction(connection, tx, [payer]);
        console.log(`Created ATA ${label}: ${ata.toBase58()} sig=${sig}`);
        await printTxLogs(sig);
      } else {
        console.log(`ATA ${label} exists: ${ata.toBase58()}`);
      }
      return ata;
    }

    async function createTokenAccountOwnedBy(mint: PublicKey, owner: PublicKey, label: string): Promise<PublicKey> {
      const tokenAccount = Keypair.generate();
      const rentLamports = await connection.getMinimumBalanceForRentExemption((splToken as any).ACCOUNT_SIZE);
      const tx = new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: payer.publicKey,
          newAccountPubkey: tokenAccount.publicKey,
          space: (splToken as any).ACCOUNT_SIZE,
          lamports: rentLamports,
          programId: TOKEN_PROGRAM_ID,
        }),
        (splToken as any).createInitializeAccountInstruction(tokenAccount.publicKey, mint, owner, TOKEN_PROGRAM_ID)
      );
      const sig = await sendAndConfirmTransaction(connection, tx, [payer, tokenAccount]);
      console.log(`Created token acct (${label}): ${tokenAccount.publicKey.toBase58()} owner=${owner.toBase58()} sig=${sig}`);
      await printTxLogs(sig);
      return tokenAccount.publicKey;
    }

    async function mintTokensTo(mint: PublicKey, destination: PublicKey, amount: number, authoritySigner: Keypair) {
      // Use any-cast to handle different spl-token versions; pass BigInt for amount if required
      const mintToFn = (splToken as any).mintTo ?? (splToken as any).mintToChecked ?? null;
      if (!mintToFn) throw new Error("spl-token mintTo function not found in this version of spl-token shim.");
      // try a few signature patterns
      try {
        // pattern: mintTo(connection, payer, mint, destination, authority, amount)
        const sig = await mintToFn(connection, authoritySigner, mint, destination, authoritySigner, BigInt(amount));
        console.log(`Minted ${amount} to ${destination.toBase58()} for mint ${mint.toBase58()} sig=${sig}`);
        await printTxLogs(sig);
        return sig;
      } catch (e1) {
        try {
          // alternative pattern: mintTo(connection, mint, destination, authority, [], amount)
          const sig = await (splToken as any).mintTo(connection, authoritySigner, mint, destination, authoritySigner.publicKey, [], BigInt(amount));
          console.log(`Minted ${amount} (alt) to ${destination.toBase58()} for mint ${mint.toBase58()} sig=${sig}`);
          await printTxLogs(sig);
          return sig;
        } catch (e2) {
          throw new Error("mintTo attempts failed: " + String(e2));
        }
      }
    }

    // ---------- Begin scenario ----------
    console.log("\n=== Step 1: Create stack & quote mints ===");
    const stackDecimals = 6;
    const quoteDecimals = 6;
    const stackMint = await createMint(stackDecimals, payer.publicKey);
    const quoteMint = await createMint(quoteDecimals, payer.publicKey);

    // PDAs - use `progId` instead of program.programId to avoid unknown property error
    const [globalPda, globalBump] = await PublicKey.findProgramAddress([Buffer.from("global")], progId);
    console.log("Global PDA:", globalPda.toBase58(), "bump:", globalBump);

    const [stackInfoPda, stackInfoBump] = await PublicKey.findProgramAddress(
      [Buffer.from("stack_info"), stackMint.toBuffer()],
      progId
    );
    console.log("StackInfo PDA:", stackInfoPda.toBase58(), "bump:", stackInfoBump);

    const [stackMintAuthPda, stackMintAuthBump] = await PublicKey.findProgramAddress(
      [Buffer.from("stack_mint_auth"), stackMint.toBuffer()],
      progId
    );
    console.log("StackMintAuth PDA:", stackMintAuthPda.toBase58(), "bump:", stackMintAuthBump);

    const [poolPda, poolBump] = await PublicKey.findProgramAddress(
      [Buffer.from("pool"), stackMint.toBuffer(), quoteMint.toBuffer()],
      progId
    );
    console.log("Pool PDA:", poolPda.toBase58(), "bump:", poolBump);

    const [vaultAuthPda, vaultAuthBump] = await PublicKey.findProgramAddress(
      [Buffer.from("vault_authority"), poolPda.toBuffer()],
      progId
    );
    console.log("VaultAuth PDA:", vaultAuthPda.toBase58(), "bump:", vaultAuthBump);

    // Treasury ATA
    console.log("\n=== Step 2: Create treasury ATA (quote mint) ===");
    const treasuryAta = await getOrCreateAtaAndLog(adminPubkey, quoteMint, "treasury (quote)");
    await ensureExists(treasuryAta, "treasury ATA");

    // Step 3: init_global
    console.log("\n=== Step 3: init_global ===");
    try {
      const protocolFeeBps = 50;
      const pauser = adminPubkey;
      const feeManager = adminPubkey;
      const governance = adminPubkey;

      const txSig = await program.methods
        .initGlobal(protocolFeeBps, pauser, feeManager, governance)
        .accounts({
          global: globalPda,
          admin: adminPubkey,
          treasury: treasuryAta,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();
      console.log("init_global tx:", txSig);
      await printTxLogs(txSig);

      const globalState: any = await program.account.global.fetch(globalPda);
      console.log("globalState:", {
        admin: globalState.admin.toBase58(),
        protocol_fee_bps: Number(globalState.protocol_fee_bps ?? globalState.protocolFeeBps),
        treasury: globalState.treasury.toBase58(),
      });
      assert.equal(globalState.admin.toBase58(), adminPubkey.toBase58());
      assert.equal(Number(globalState.protocol_fee_bps ?? globalState.protocolFeeBps), protocolFeeBps);
    } catch (err) {
      console.error("init_global failed:", err);
      throw err;
    }

    // Step 4: register_stack
    console.log("\n=== Step 4: register_stack ===");
    try {
      const creatorFeeBps = 300;
      const txSig = await program.methods
        .registerStack(creatorFeeBps)
        .accounts({
          creator: adminPubkey,
          stackMint: stackMint,
          stackMintAuth: stackMintAuthPda,
          stackInfo: stackInfoPda,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();
      console.log("register_stack tx:", txSig);
      await printTxLogs(txSig);

      const stackInfo: any = await program.account.stackInfo.fetch(stackInfoPda);
      console.log("stackInfo:", {
        creator: stackInfo.creator.toBase58(),
        stack_mint: stackInfo.stackMint?.toBase58?.() ?? stackInfo.stack_mint?.toBase58?.(),
        creator_fee_bps: Number(stackInfo.creator_fee_bps ?? stackInfo.creatorFeeBps),
      });
      assert.equal(stackInfo.creator.toBase58(), adminPubkey.toBase58());
      assert.equal(Number(stackInfo.creator_fee_bps ?? stackInfo.creatorFeeBps), creatorFeeBps);
    } catch (err) {
      console.error("register_stack failed:", err);
      throw err;
    }

    // Step 5: LP mint (authority = vault PDA)
    console.log("\n=== Step 5: create LP mint (authority = vault PDA) ===");
    let lpMint: PublicKey;
    try {
      lpMint = await createMint(9, vaultAuthPda);
      await ensureExists(lpMint, "lp mint");
    } catch (err) {
      console.error("lp mint creation failed:", err);
      throw err;
    }

    // Step 6: vault token accounts (owned by vault PDA)
    console.log("\n=== Step 6: create vault token accounts (owned by vault PDA) ===");
    let stackVault: PublicKey;
    let quoteVault: PublicKey;
    let protocolFeeVault: PublicKey;
    let creatorFeeVault: PublicKey;
    try {
      stackVault = await createTokenAccountOwnedBy(stackMint, vaultAuthPda, "stack_vault");
      quoteVault = await createTokenAccountOwnedBy(quoteMint, vaultAuthPda, "quote_vault");
      protocolFeeVault = await createTokenAccountOwnedBy(quoteMint, vaultAuthPda, "protocol_fee_vault");
      creatorFeeVault = await createTokenAccountOwnedBy(quoteMint, vaultAuthPda, "creator_fee_vault");

      await Promise.all([
        ensureExists(stackVault, "stack_vault"),
        ensureExists(quoteVault, "quote_vault"),
        ensureExists(protocolFeeVault, "protocol_fee_vault"),
        ensureExists(creatorFeeVault, "creator_fee_vault"),
      ]);
    } catch (err) {
      console.error("Failed creating vault token accounts:", err);
      throw err;
    }

    // Step 7: create_pool
    console.log("\n=== Step 7: create_pool ===");
    try {
      const feeBps = 30;
      const kValBN = new BN("1000000000000000000");
      const feeOnTransfer = false;
      const decimalNormalizeTo = 9;

      // sanity checks before call
      await ensureExists(stackMint, "stackMint");
      await ensureExists(quoteMint, "quoteMint");
      await ensureExists(lpMint, "lpMint");
      await ensureExists(stackVault, "stackVault");
      await ensureExists(quoteVault, "quoteVault");
      await ensureExists(protocolFeeVault, "protocolFeeVault");
      await ensureExists(creatorFeeVault, "creatorFeeVault");
      await ensureExists(globalPda, "globalPda");

      const txSig = await program.methods
        .createPool(feeBps, kValBN, feeOnTransfer, decimalNormalizeTo)
        .accounts({
          creator: adminPubkey,
          stackMint: stackMint,
          quoteMint: quoteMint,
          lpMint: lpMint,
          pool: poolPda,
          stackVault: stackVault,
          quoteVault: quoteVault,
          protocolFeeVault: protocolFeeVault,
          creatorFeeVault: creatorFeeVault,
          vaultAuthority: vaultAuthPda,
          oracle: Keypair.generate().publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
          global: globalPda,
        })
        .rpc();
      console.log("create_pool tx:", txSig);
      await printTxLogs(txSig);

      const poolState: any = await program.account.pool.fetch(poolPda);
      console.log("poolState:", {
        stack_mint: poolState.stackMint?.toBase58?.() ?? poolState.stack_mint?.toBase58?.(),
        quote_mint: poolState.quoteMint?.toBase58?.() ?? poolState.quote_mint?.toBase58?.(),
        fee_bps: Number(poolState.fee_bps ?? poolState.feeBps),
        lp_mint: (poolState.lpMint?.toBase58?.() ?? poolState.lp_mint?.toBase58?.()),
      });
      assert.equal(poolState.stackMint.toBase58(), stackMint.toBase58());
      assert.equal(poolState.quoteMint.toBase58(), quoteMint.toBase58());
    } catch (err) {
      console.error("create_pool failed:", err);
      throw err;
    }

    // Step 8: create user ATAs and mint tokens to user
    console.log("\n=== Step 8: mint tokens to user & create user ATAs ===");
    const userStackAta = await getOrCreateAtaAndLog(adminPubkey, stackMint, "user_stack");
    const userQuoteAta = await getOrCreateAtaAndLog(adminPubkey, quoteMint, "user_quote");

    await mintTokensTo(stackMint, userStackAta, 1_000_000, payer);
    await mintTokensTo(quoteMint, userQuoteAta, 2_000_000, payer);

    // Step 9: provide_liquidity
    console.log("\n=== Step 9: provide_liquidity ===");
    try {
      const userLpAta = await getOrCreateAtaAndLog(adminPubkey, lpMint, "user_lp_account");
      const amountStack = new BN(100_000);
      const amountQuote = new BN(200_000);

      const txSig = await program.methods
        .provideLiquidity(amountStack, amountQuote)
        .accounts({
          user: adminPubkey,
          pool: poolPda,
          stackMint: stackMint,
          quoteMint: quoteMint,
          stackVault: stackVault,
          quoteVault: quoteVault,
          protocolFeeVault: protocolFeeVault,
          creatorFeeVault: creatorFeeVault,
          lpMint: lpMint,
          userLpAccount: userLpAta,
          userStackAccount: userStackAta,
          userQuoteAccount: userQuoteAta,
          treasuryTokenAccount: treasuryAta,
          vaultAuthority: vaultAuthPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          global: globalPda,
        })
        .rpc();
      console.log("provide_liquidity tx:", txSig);
      await printTxLogs(txSig);

      const poolState: any = await program.account.pool.fetch(poolPda);
      console.log("pool.total_lp_supply:", poolState.total_lp_supply ?? poolState.totalLpSupply);
      assert(Number(poolState.total_lp_supply ?? poolState.totalLpSupply) > 0);

      const userLpBalance = await connection.getTokenAccountBalance(userLpAta);
      console.log("user LP balance:", userLpBalance.value.amount);
      assert(Number(userLpBalance.value.amount) > 0, "LP minted to user");
    } catch (err) {
      console.error("provide_liquidity failed:", err);
      throw err;
    }

    // Step 10: swap stack -> quote
    console.log("\n=== Step 10: swap_stack_to_quote ===");
    try {
      const swapIn = 1_000;
      const minOut = 1;

      const txSig = await program.methods
        .swapStackToQuote(new BN(swapIn), new BN(minOut), null, false)
        .accounts({
          user: adminPubkey,
          pool: poolPda,
          stackMint: stackMint,
          quoteMint: quoteMint,
          stackVault: stackVault,
          quoteVault: quoteVault,
          userStackAccount: userStackAta,
          userQuoteAccount: userQuoteAta,
          protocolFeeVault: protocolFeeVault,
          creatorFeeVault: creatorFeeVault,
          treasuryTokenAccount: treasuryAta,
          vaultAuthority: vaultAuthPda,
          stackInfo: stackInfoPda,
          governanceApproval: null,
          tokenProgram: TOKEN_PROGRAM_ID,
          global: globalPda,
        })
        .rpc();
      console.log("swap tx:", txSig);
      await printTxLogs(txSig);

      const quoteBalAfter = await connection.getTokenAccountBalance(userQuoteAta);
      console.log("user quote ATA balance (after):", quoteBalAfter.value.amount);
    } catch (err) {
      console.error("swap_stack_to_quote failed:", err);
      throw err;
    }

    // Step 11: remove_liquidity (burn half)
    console.log("\n=== Step 11: remove_liquidity ===");
    try {
      const userLpAta = await (splToken as any).getAssociatedTokenAddress(lpMint, adminPubkey, false, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);
      const lpBal = await connection.getTokenAccountBalance(userLpAta);
      const lpAmount = Number(lpBal.value.amount);
      if (lpAmount === 0) throw new Error("No LP tokens to burn for remove_liquidity");
      const removeAmount = Math.floor(lpAmount / 2);

      const txSig = await program.methods
        .removeLiquidity(new BN(removeAmount))
        .accounts({
          user: adminPubkey,
          pool: poolPda,
          lpMint: lpMint,
          userLpAccount: userLpAta,
          stackVault: stackVault,
          quoteVault: quoteVault,
          stackMint: stackMint,
          quoteMint: quoteMint,
          userStackAccount: userStackAta,
          userQuoteAccount: userQuoteAta,
          treasuryTokenAccount: treasuryAta,
          vaultAuthority: vaultAuthPda,
          tokenProgram: TOKEN_PROGRAM_ID,
          global: globalPda,
        })
        .rpc();
      console.log("remove_liquidity tx:", txSig);
      await printTxLogs(txSig);

      const poolState = await program.account.pool.fetch(poolPda);
      console.log("pool.total_lp_supply (after):", poolState.total_lp_supply ?? poolState.totalLpSupply);
      const userLpBalAfter = await connection.getTokenAccountBalance(userLpAta);
      console.log("user LP balance (after):", userLpBalAfter.value.amount);
    } catch (err) {
      console.error("remove_liquidity failed:", err);
      throw err;
    }

    // Optional: claim_creator_fees (non-fatal)
    console.log("\n=== Optional: claim_creator_fees (diagnostic) ===");
    try {
      const poolStateAny: any = await program.account.pool.fetch(poolPda);
      const creatorClaimable = Number(poolStateAny.creator_claimable ?? poolStateAny.creatorClaimable ?? 0);
      console.log("creator_claimable (normalized):", creatorClaimable);
      if (creatorClaimable > 0) {
        const creatorReceiver = await getOrCreateAtaAndLog(adminPubkey, quoteMint, "creator_receiver");
        const txSig = await program.methods
          .claimCreatorFees()
          .accounts({
            creator: adminPubkey,
            pool: poolPda,
            stackInfo: stackInfoPda,
            quoteMint: quoteMint,
            creatorFeeVault: creatorFeeVault,
            creatorReceiver: creatorReceiver,
            vaultAuthority: vaultAuthPda,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .rpc();
        console.log("claim_creator_fees tx:", txSig);
        await printTxLogs(txSig);
      } else {
        console.log("no creator_claimable; skipping claim");
      }
    } catch (err) {
      console.warn("claim_creator_fees encountered error (non-fatal):", err);
    }

    // Final summary
    console.log("\n=== Final state summary ===");
    try {
      const finalGlobal = await program.account.global.fetch(globalPda);
      const finalPool = await program.account.pool.fetch(poolPda);
      console.log("Final global:", {
        admin: finalGlobal.admin.toBase58(),
        protocol_fee_bps: Number(finalGlobal.protocol_fee_bps ?? finalGlobal.protocolFeeBps),
        paused: finalGlobal.paused,
      });
      console.log("Final pool:", {
        stack_mint: finalPool.stackMint?.toBase58?.() ?? finalPool.stack_mint?.toBase58?.(),
        quote_mint: finalPool.quoteMint?.toBase58?.() ?? finalPool.quote_mint?.toBase58?.(),
        total_lp_supply: finalPool.total_lp_supply ?? finalPool.totalLpSupply,
        creator_claimable: finalPool.creator_claimable ?? finalPool.creatorClaimable,
      });
    } catch (err) {
      console.warn("Unable to fetch final state (non-fatal):", err);
    }

    console.log("Diagnostics test finished.");
  });
});
