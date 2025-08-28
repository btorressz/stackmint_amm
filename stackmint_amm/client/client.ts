// client.ts — Playground client for stackmint_amm
// Define runClient(pg) with no exports and no use of banned globals.

/* eslint-disable @typescript-eslint/no-explicit-any */
import * as anchor from "@project-serum/anchor";
import {
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import * as splToken from "@solana/spl-token";

const PROGRAM_ID = new PublicKey("7zcYfbAQNpGXpkfn5tXh7zMhJzm5UkQJeLbv2871cjVt");

const CREATE_NEW_MINTS = true;
const STACK_DECIMALS = 6;
const QUOTE_DECIMALS = 6;

async function runClient(pg: any): Promise<void> {
  if (!pg || !pg.connection || !pg.wallet) {
    throw new Error("Playground `pg` object missing. Run inside Solana Playground.");
  }

  const connection: Connection = pg.connection as Connection;

  const AnchorProviderCtor: any = (anchor as any).AnchorProvider ?? (anchor as any).Provider;
  if (!AnchorProviderCtor) throw new Error("Cannot find Anchor Provider/Provider.");

  const defaultOptions =
    typeof AnchorProviderCtor.defaultOptions === "function" ? AnchorProviderCtor.defaultOptions() : {};

  const provider = new AnchorProviderCtor(connection, pg.wallet, defaultOptions);
  anchor.setProvider(provider);

  console.log("Provider ready.");
  console.log("Program ID:", PROGRAM_ID.toBase58());
  console.log("Wallet:", provider.wallet.publicKey.toBase58());
  try {
    const bal = await connection.getBalance(provider.wallet.publicKey);
    console.log(`Balance: ${bal / LAMPORTS_PER_SOL} SOL`);
  } catch (err) {
    console.warn("Could not fetch balance:", err);
  }

  let program: any | undefined;

  try {
    const maybeProg = (pg as any).program;
    if (maybeProg && maybeProg.programId && typeof maybeProg.programId.equals === "function") {
      if (maybeProg.programId.equals(PROGRAM_ID)) {
        program = maybeProg;
        console.log("Using program from Playground.");
      } else {
        console.warn("Playground program present but id differs.");
      }
    }
  } catch {
    // continue
  }

  if (!program) {
    try {
      const idl = await (anchor as any).Program.fetchIdl(PROGRAM_ID, provider);
      if (idl) {
        program = new (anchor as any).Program(idl, PROGRAM_ID, provider);
        console.log("Program built from on-chain IDL.");
      } else {
        console.warn("No IDL on-chain for PROGRAM_ID.");
      }
    } catch (err) {
      console.warn("Failed fetchIdl:", err);
    }
  }

  if (!program) {
    try {
      const ws = (anchor as any).workspace ?? {};
      for (const [, p] of Object.entries(ws)) {
        try {
          if ((p as any).programId && typeof (p as any).programId.equals === "function") {
            if ((p as any).programId.equals(PROGRAM_ID)) {
              program = p;
              console.log("Found program in workspace.");
              break;
            }
          }
        } catch {
          // ignore
        }
      }
    } catch {
      // ignore
    }
  }

  if (!program) {
    throw new Error("Unable to resolve program. Provide IDL or expose pg.program.");
  }

  async function printLogs(sig: string | null | undefined) {
    if (!sig) return;
    try {
      const tx = await connection.getTransaction(sig, { commitment: "confirmed" });
      console.log(`=== logs for ${sig} ===`);
      (tx?.meta?.logMessages ?? []).forEach((l: string) => console.log("   ", l));
      console.log("=== end logs ===");
    } catch (e) {
      console.warn("Error fetching logs:", e);
    }
  }

  async function ensureAirdrop(kp: Keypair, sol = 1) {
    try {
      const sig = await connection.requestAirdrop(kp.publicKey, sol * LAMPORTS_PER_SOL);
      await connection.confirmTransaction(sig, "confirmed");
      console.log("Airdrop to helper:", kp.publicKey.toBase58(), "sig:", sig);
    } catch (e) {
      console.warn("Airdrop not needed or failed:", e);
    }
  }

  async function createMintHelper(decimals: number, mintAuth: PublicKey): Promise<PublicKey> {
    const payer = Keypair.generate();
    await ensureAirdrop(payer, 1);
    try {
      if (typeof (splToken as any).createMint === "function") {
        const mint = await (splToken as any).createMint(connection, payer, mintAuth, null, decimals);
        console.log(`Created mint ${mint.toBase58()} decimals=${decimals}`);
        return mint;
      } else if ((splToken as any).Token && typeof (splToken as any).Token.createMint === "function") {
        const token = await (splToken as any).Token.createMint(
          connection,
          payer,
          mintAuth,
          null,
          decimals,
          (splToken as any).TOKEN_PROGRAM_ID
        );
        console.log(`Created mint (legacy) ${token.publicKey.toBase58()}`);
        return token.publicKey;
      } else {
        throw new Error("No createMint API in @solana/spl-token.");
      }
    } catch (err) {
      console.error("createMintHelper failed:", err);
      throw err;
    }
  }

  async function getOrCreateAta(owner: PublicKey, mint: PublicKey): Promise<PublicKey> {
    const TOKEN_PROGRAM_ID = (splToken as any).TOKEN_PROGRAM_ID ?? splToken.TOKEN_PROGRAM_ID;
    const ASSOCIATED_TOKEN_PROGRAM_ID =
      (splToken as any).ASSOCIATED_TOKEN_PROGRAM_ID ?? splToken.ASSOCIATED_TOKEN_PROGRAM_ID;

    let ata: PublicKey;
    if (typeof (splToken as any).getAssociatedTokenAddress === "function") {
      try {
        ata = await (splToken as any).getAssociatedTokenAddress(mint, owner);
      } catch {
        ata = await (splToken as any).getAssociatedTokenAddress(
          mint,
          owner,
          false,
          TOKEN_PROGRAM_ID,
          ASSOCIATED_TOKEN_PROGRAM_ID
        );
      }
    } else {
      const [p] = await PublicKey.findProgramAddress(
        [owner.toBuffer(), (TOKEN_PROGRAM_ID as PublicKey).toBuffer(), mint.toBuffer()],
        ASSOCIATED_TOKEN_PROGRAM_ID
      );
      ata = p;
    }

    const info = await connection.getAccountInfo(ata);
    if (!info) {
      let ix: any | null = null;
      if (typeof (splToken as any).createAssociatedTokenAccountInstruction === "function") {
        try {
          ix = (splToken as any).createAssociatedTokenAccountInstruction(provider.wallet.publicKey, ata, owner, mint);
        } catch {
          ix = (splToken as any).createAssociatedTokenAccountInstruction(
            provider.wallet.publicKey,
            ata,
            owner,
            mint,
            TOKEN_PROGRAM_ID,
            ASSOCIATED_TOKEN_PROGRAM_ID
          );
        }
      }

      if (!ix) {
        const keys = [
          { pubkey: provider.wallet.publicKey, isSigner: true, isWritable: true },
          { pubkey: ata, isSigner: false, isWritable: true },
          { pubkey: owner, isSigner: false, isWritable: false },
          { pubkey: mint, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          { pubkey: TOKEN_PROGRAM_ID as PublicKey, isSigner: false, isWritable: false },
          { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
        ];
        ix = new anchor.web3.TransactionInstruction({
          keys,
          programId: ASSOCIATED_TOKEN_PROGRAM_ID as PublicKey,
          data: Buffer.from([]),
        });
      }

      const tx = new Transaction().add(ix);
      try {
        if (typeof provider.sendAndConfirm === "function") {
          const sig = await provider.sendAndConfirm(tx);
          console.log(`Created ATA ${ata.toBase58()} sig=${sig}`);
          await printLogs(sig);
        } else if (typeof provider.send === "function") {
          const sig = await provider.send(tx);
          console.log(`Created ATA ${ata.toBase58()} sig=${sig}`);
          await printLogs(sig);
        } else {
          const signed = await provider.wallet.signTransaction(tx);
          const raw = signed.serialize();
          const sig = await connection.sendRawTransaction(raw);
          await connection.confirmTransaction(sig, "confirmed");
          console.log(`Created ATA (raw) ${ata.toBase58()} sig=${sig}`);
          await printLogs(sig);
        }
      } catch (e) {
        console.error("Failed to create ATA:", e);
        throw e;
      }
    } else {
      console.log(`ATA exists: ${ata.toBase58()}`);
    }
    return ata;
  }

  const [globalPda] = await PublicKey.findProgramAddress([Buffer.from("global")], PROGRAM_ID);
  console.log("Global PDA:", globalPda.toBase58());

  let stackMint: PublicKey;
  let quoteMint: PublicKey;
  if (CREATE_NEW_MINTS) {
    console.log("Creating mints.");
    const mintAuth = provider.wallet.publicKey;
    stackMint = await createMintHelper(STACK_DECIMALS, mintAuth);
    quoteMint = await createMintHelper(QUOTE_DECIMALS, mintAuth);
  } else {
    throw new Error("CREATE_NEW_MINTS=false but no existing mint addresses provided.");
  }

  const treasuryAta = await getOrCreateAta(provider.wallet.publicKey, quoteMint);

  const protocolFeeBps = 50;
  const pauser = provider.wallet.publicKey;
  const feeManager = provider.wallet.publicKey;
  const governance = provider.wallet.publicKey;
  const maxFeeBps = 2000;
  const dustThreshold = 10;
  const creatorClaimLockSecs = 60 * 60 * 24 * 7;

  try {
    console.log("Calling init_global...");
    if (program?.methods?.initGlobal) {
      const sig = await program.methods
        .initGlobal(
          protocolFeeBps,
          pauser,
          feeManager,
          governance,
          maxFeeBps,
          dustThreshold,
          creatorClaimLockSecs
        )
        .accounts({
          global: globalPda,
          admin: provider.wallet.publicKey,
          treasury: treasuryAta,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();
      console.log("init_global tx:", sig);
      await printLogs(sig);
    } else if (program?.rpc && typeof program.rpc === "function") {
      const sig = await program.rpc.init_global(
        protocolFeeBps,
        pauser,
        feeManager,
        governance,
        maxFeeBps,
        dustThreshold,
        creatorClaimLockSecs,
        {
          accounts: {
            global: globalPda,
            admin: provider.wallet.publicKey,
            treasury: treasuryAta,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          },
        }
      );
      console.log("init_global tx (rpc):", sig);
      await printLogs(sig);
    } else {
      throw new Error("Program does not expose expected entry for init_global.");
    }
  } catch (err) {
    console.error("init_global failed:", err);
    throw err;
  }

  console.log("Setup complete. Now you can run registration, pool create, and liquidity actions.");
}
