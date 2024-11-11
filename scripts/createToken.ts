import anchor from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";
import * as spl from "@solana/spl-token";

const connection = new anchor.web3.Connection(
  process.env.RPC_URL ?? "",
  "confirmed"
);
const wallet = anchor.Wallet.local();

const tokenKeypair = Keypair.generate();
const token = tokenKeypair.publicKey;

await spl.createMint(
  connection,
  wallet.payer,
  wallet.publicKey,
  null,
  6,
  tokenKeypair
);

console.log("Token created:", token.toBase58());

const userAta = await spl.createAssociatedTokenAccount(
  connection,
  wallet.payer,
  token,
  wallet.publicKey
);

console.log("Associated token account created:", userAta.toBase58());

await spl.mintTo(
  connection,
  wallet.payer,
  token,
  userAta,
  wallet.publicKey,
  100_000_000e6
);

console.log("Minted 100,000,000 tokens to:", wallet.publicKey.toBase58());
