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

const config = new PublicKey(process.env.TOKEN_MILL_CONFIG ?? "");
const quoteTokenMint = new PublicKey(process.env.QUOTE_TOKEN ?? "");
const market = new PublicKey(process.env.MARKET ?? "");

const marketAccount = await program.account.market.fetch(market);
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

const userReferralAccount = PublicKey.findProgramAddressSync(
  [Buffer.from("referral"), config.toBuffer(), wallet.publicKey.toBuffer()],
  program.programId
)[0];

const referralAccountAta = await spl.getOrCreateAssociatedTokenAccount(
  connection,
  wallet.payer,
  quoteTokenMint,
  userReferralAccount,
  true
);

const userReferralAccountInfo = await connection.getAccountInfo(
  userReferralAccount
);

if (!userReferralAccountInfo) {
  const transaction = await program.methods
    .createReferralAccount(wallet.publicKey)
    .accountsPartial({
      config,
      referralAccount: userReferralAccount,
      user: wallet.publicKey,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Referral account creation failed:", result.value.err);
    process.exit(1);
  }

  console.log("Referral account created");
}

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
      market,
      baseTokenMint,
      quoteTokenMint,
      marketBaseTokenAta,
      marketQuoteTokenAta,
      userBaseTokenAta,
      userQuoteTokenAta,
      protocolQuoteTokenAta: userQuoteTokenAta,
      referralTokenAccount: referralAccountAta.address,
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
    .claimReferralFees()
    .accountsPartial({
      referralAccount: userReferralAccount,
      quoteTokenMint,
      referralAccountQuoteTokenAta: referralAccountAta.address,
      referrerQuoteTokenAta: userQuoteTokenAta,
      referrer: wallet.publicKey,
      quoteTokenProgram: spl.TOKEN_PROGRAM_ID,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Referral fees claim failed:", result.value.err);
    process.exit(1);
  }

  console.log("Referral fees claim created");
}
