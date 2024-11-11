import anchor, { BN, Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
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

const quoteTokenMint = new PublicKey(process.env.QUOTE_TOKEN ?? "");
const market = new PublicKey(process.env.MARKET ?? "");

const marketAccount = await program.account.market.fetch(market);
const config = marketAccount.config;
const baseTokenMint = marketAccount.baseTokenMint;

const marketBaseTokenAta = spl.getAssociatedTokenAddressSync(
  baseTokenMint,
  market,
  true,
  spl.TOKEN_2022_PROGRAM_ID
);

const userBaseTokenAta = spl.getAssociatedTokenAddressSync(
  baseTokenMint,
  wallet.publicKey,
  true,
  spl.TOKEN_2022_PROGRAM_ID
);

const marketQuoteTokenAta = spl.getAssociatedTokenAddressSync(
  quoteTokenMint,
  market,
  true
);

const userQuoteTokenAta = spl.getAssociatedTokenAddressSync(
  quoteTokenMint,
  wallet.publicKey
);

const u64Max = new BN(2).pow(new BN(64)).sub(new BN(1));

const swapActions = [];
swapActions.push([{ buy: {} }, { exactOutput: {} }, new BN(100e6), u64Max]);
swapActions.push([{ sell: {} }, { exactInput: {} }, new BN(50e6), new BN(0)]);
swapActions.push([{ buy: {} }, { exactOutput: {} }, new BN(300e6), u64Max]);
swapActions.push([{ buy: {} }, { exactOutput: {} }, new BN(432e6), u64Max]);
swapActions.push([{ sell: {} }, { exactInput: {} }, new BN(100e6), new BN(0)]);

for (const action of swapActions) {
  const transaction = await program.methods
    .swap(...action)
    .accountsPartial({
      config,
      market,
      baseTokenMint,
      quoteTokenMint,
      marketBaseTokenAta,
      marketQuoteTokenAta,
      userBaseTokenAta,
      userQuoteTokenAta,
      protocolQuoteTokenAta: userQuoteTokenAta, // Here protocol fee recipient is the user
      referralTokenAccount: program.programId,
      user: wallet.publicKey,
      baseTokenProgram: spl.TOKEN_2022_PROGRAM_ID,
      quoteTokenProgram: spl.TOKEN_PROGRAM_ID,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Swap failed:", result.value.err);
    process.exit(1);
  }

  console.log("Swap complete");
}

{
  const transaction = await program.methods
    .claimCreatorFees()
    .accountsPartial({
      market,
      quoteTokenMint,
      marketQuoteTokenAta,
      creatorQuoteTokenAta: userQuoteTokenAta,
      creator: wallet.publicKey,
      quoteTokenProgram: spl.TOKEN_PROGRAM_ID,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Creator fees claim failed:", result.value.err);
    process.exit(1);
  }

  console.log("Creator fees claimed");
}
