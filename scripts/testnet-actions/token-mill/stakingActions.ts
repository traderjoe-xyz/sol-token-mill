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

const staking = PublicKey.findProgramAddressSync(
  [Buffer.from("market_staking"), market.toBuffer()],
  program.programId
)[0];

const stakingAccountInfo = await connection.getAccountInfo(staking);

if (!stakingAccountInfo) {
  const transaction = await program.methods
    .createStaking()
    .accountsPartial({
      market,
      staking,
      payer: wallet.publicKey,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Staking activation failed:", result.value.err);
    process.exit(1);
  }

  console.log("Staking enabled");
}

const stakePosition = PublicKey.findProgramAddressSync(
  [
    Buffer.from("stake_position"),
    market.toBuffer(),
    wallet.publicKey.toBuffer(),
  ],
  program.programId
)[0];

const stakePositionAccountInfo = await connection.getAccountInfo(stakePosition);

if (!stakePositionAccountInfo) {
  const transaction = await program.methods
    .createStakePosition()
    .accountsPartial({
      market,
      stakePosition,
      user: wallet.publicKey,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Stake position creation failed:", result.value.err);
    process.exit(1);
  }

  console.log("Stake position created");
}

const u64Max = new BN(2).pow(new BN(64)).sub(new BN(1));

const swapActions = [];
swapActions.push([{ buy: {} }, { exactOutput: {} }, new BN(100e6), u64Max]);

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
    console.log("Initial buy failed:", result);
    process.exit(1);
  }

  console.log("Initial buy complete");
}

{
  const transaction = await program.methods
    .deposit(new BN(100e6))
    .accountsPartial({
      market,
      staking,
      stakePosition,
      marketBaseTokenAta,
      userBaseTokenAta,
      baseTokenMint,
      baseTokenProgram: spl.TOKEN_2022_PROGRAM_ID,
      user: wallet.publicKey,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Deposit failed:", result.value.err);
    process.exit(1);
  }

  console.log("Deposit successful");
}

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
    .withdraw(new BN(100e6))
    .accountsPartial({
      market,
      staking,
      stakePosition,
      marketBaseTokenAta,
      userBaseTokenAta,
      baseTokenMint,
      baseTokenProgram: spl.TOKEN_2022_PROGRAM_ID,
      user: wallet.publicKey,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Withdrawal failed:", result.value.err);
    process.exit(1);
  }

  console.log("Withdrawal successful");
}

{
  const transaction = await program.methods
    .claimStakingRewards()
    .accountsPartial({
      market,
      staking,
      stakePosition,
      marketQuoteTokenAta,
      userQuoteTokenAta,
      quoteTokenMint,
      quoteTokenProgram: spl.TOKEN_PROGRAM_ID,
      user: wallet.publicKey,
    })
    .signers([wallet.payer])
    .transaction();

  const transactionSignature = await connection.sendTransaction(transaction, [
    wallet.payer,
  ]);

  const result = await connection.confirmTransaction(transactionSignature);

  if (result.value.err) {
    console.log("Claim staking rewards failed:", result.value.err);
    process.exit(1);
  }

  console.log("Claim staking rewards successful");
}
