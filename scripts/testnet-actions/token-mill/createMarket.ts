import anchor, { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";
import * as spl from "@solana/spl-token";

import { type TokenMill } from "../../../target/types/token_mill";
import TokenMillIdl from "../../../target/idl/token_mill.json";

const connection = new anchor.web3.Connection(
  process.env.RPC_URL ?? "",
  "confirmed"
);

const wallet = anchor.Wallet.local();

const program = new Program<TokenMill>(TokenMillIdl as any, {
  connection,
});

const config = new PublicKey(process.env.TOKEN_MILL_CONFIG ?? "");
const quoteTokenMint = new PublicKey(process.env.QUOTE_TOKEN ?? "");

const quoteTokenBadge = PublicKey.findProgramAddressSync(
  [
    Buffer.from("quote_token_badge"),
    config.toBuffer(),
    quoteTokenMint.toBuffer(),
  ],
  program.programId
)[0];

const quoteTokenBadgeAccount = await connection.getAccountInfo(quoteTokenBadge);

if (!quoteTokenBadgeAccount) {
  console.log("Creating quote token badge");

  const transaction = await program.methods
    .createQuoteAssetBadge()
    .accountsPartial({
      config,
      tokenMint: quoteTokenMint,
      authority: wallet.publicKey,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  await connection.confirmTransaction(transactionSignature);

  console.log("Quote token badge created");
}

const baseTokenKeypair = Keypair.generate();
const baseTokenMint = baseTokenKeypair.publicKey;

const market = PublicKey.findProgramAddressSync(
  [Buffer.from("market"), baseTokenMint.toBuffer()],
  program.programId
)[0];

const marketBaseTokenAta = spl.getAssociatedTokenAddressSync(
  baseTokenMint,
  market,
  true,
  spl.TOKEN_2022_PROGRAM_ID
);

{
  const transaction = await program.methods
    .createMarket("Test Market", "TM", "", new BN(1_000_000e6), 3_000, 4_000)
    .accountsPartial({
      config,
      market,
      baseTokenMint,
      marketBaseTokenAta,
      quoteTokenBadge,
      quoteTokenMint,
      creator: wallet.publicKey,
    })
    .signers([wallet.payer, baseTokenKeypair])
    .transaction();

  const transactionSignature = await connection.sendTransaction(
    transaction,
    [wallet.payer, baseTokenKeypair],
    {
      skipPreflight: true,
    }
  );

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Market creation failed:", result.value.err);
    process.exit(1);
  }
}

console.log("Market created:", market.toBase58());

await spl.getOrCreateAssociatedTokenAccount(
  connection,
  wallet.payer,
  baseTokenMint,
  wallet.publicKey,
  true,
  "confirmed",
  undefined,
  spl.TOKEN_2022_PROGRAM_ID
);

await spl.getOrCreateAssociatedTokenAccount(
  connection,
  wallet.payer,
  quoteTokenMint,
  market,
  true
);

console.log("Market ATAs created");

await spl.getOrCreateAssociatedTokenAccount(
  connection,
  wallet.payer,
  baseTokenMint,
  wallet.publicKey,
  true,
  "confirmed",
  undefined,
  spl.TOKEN_2022_PROGRAM_ID
);

console.log("User base token ATA created");

{
  const bidPrices = [];
  const askPrices = [];

  for (let i = 0; i < 11; i++) {
    bidPrices.push(new BN(i * 9e5));
    askPrices.push(new BN(i * 1e6));
  }

  const transaction = await program.methods
    .setMarketPrices(bidPrices, askPrices)
    .accountsPartial({
      market,
      creator: wallet.publicKey,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(
    transaction,
    [wallet.payer],
    {
      skipPreflight: true,
    }
  );

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Set prices failed:", result.value.err);
    process.exit(1);
  }
}

console.log("Prices set");
