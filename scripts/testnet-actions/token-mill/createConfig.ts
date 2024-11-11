import anchor, { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";

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

const config = Keypair.generate();

const transaction = await program.methods
  .createConfig(wallet.publicKey, wallet.publicKey, 2_000, 5_000)
  .accountsPartial({ config: config.publicKey, payer: wallet.publicKey })
  .signers([wallet.payer, config])
  .transaction();

const transactionSignature = await connection.sendTransaction(transaction, [
  wallet.payer,
  config,
]);

await connection.confirmTransaction(transactionSignature);

console.log("Config created:", config.publicKey.toBase58());

const wSol = new PublicKey("So11111111111111111111111111111111111111112");
const wSolAccount = await connection.getAccountInfo(wSol);

if (wSolAccount) {
  const wSolQuoteTokenBadge = PublicKey.findProgramAddressSync(
    [
      Buffer.from("quote_token_badge"),
      config.publicKey.toBuffer(),
      wSol.toBuffer(),
    ],
    program.programId
  )[0];

  const wSolQuoteTokenBadgeAccount = await connection.getAccountInfo(
    wSolQuoteTokenBadge
  );

  if (!wSolQuoteTokenBadgeAccount) {
    console.log("Creating quote token badge for wSol");

    const transaction = await program.methods
      .createQuoteAssetBadge()
      .accountsPartial({
        config: config.publicKey,
        tokenMint: wSol,
        authority: wallet.publicKey,
      })
      .signers([wallet.payer])
      .transaction();

    const transactionSignature = await connection.sendTransaction(transaction, [
      wallet.payer,
    ]);

    await connection.confirmTransaction(transactionSignature);

    console.log("wSol quote token badge created");
  }
}
