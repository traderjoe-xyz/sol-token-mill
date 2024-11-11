# Joelana

LFJ's Solana smart contracts. Javascript scripts work with [Bun](https://bun.sh/), to install dependencies, run `bun install`.

## Token Mill

Token launchpad using customizable bonding curve. Includes fee sharing (to protocol, creator, referrals and staking), along with token vesting. Solidity implementation can be found at https://github.com/traderjoe-xyz/token-mill.

### Testing

Unit testing is done using [litesvm](https://github.com/LiteSVM/litesvm). Swap, fee calculations and staking operations are also compared to their EVM counterparts using [revm](https://github.com/bluealloy/revm).
