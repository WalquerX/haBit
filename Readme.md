# Habit Tracker NFT

A Bitcoin NFT-based habit tracker built with [Charms](https://github.com/CharmsDev/charms). Track your habits on-chain with incrementing session counters stored as NFT metadata.

## Features

- 🗡️ Create habit tracker NFTs with custom habit names
- 📊 Increment session counter on each habit completion
- 👀 View NFT metadata (habit name, total sessions)
- 🔐 Supports both CLI and HTTP API interfaces
- ⚡ Works on testnet4 and regtest (automated  tests)

Note: Currently this project is WIP, then the charms app/contract is jut a placeholder that lets any modification pass.

## Quick Start

### Prerequisites

```bash
# Build contract WASM (This uses charms cli)
make contract

# Run Bitcoin testnet4 node
bitcoind -testnet4 -txindex=1 -blocksonly=1 -listen=0 -dbcache=300 -daemon
```

### CLI Usage (The CLI needs a testnet4 node running and wallet loaded)

```bash
# Create a new habit tracker
cargo run -- create --habit "Morning Meditation"

# Update (increment session counter)
cargo run -- update --utxo 

# View NFT details
cargo run -- view --utxo 
```

### API Server

```bash
# Start server
cargo run

# Server runs on http://127.0.0.1:3000
```

#### API Endpoints (Untested yet)

```bash
# Create unsigned NFT transactions
POST /api/nft/create/unsigned
{
  "habit": "Morning Meditation",
  "address": "bc1q...",
  "funding_utxo": "txid:vout",
  "funding_value": 100000
}

# Update NFT (increment sessions)
POST /api/nft/update/unsigned
{
  "nft_utxo": "txid:vout",
  "user_address": "bc1q...",
  "funding_utxo": "txid:vout",
  "funding_value": 100000
}

# Broadcast signed transactions
POST /api/nft/broadcast
{
  "signed_commit_hex": "...",
  "signed_spell_hex": "..."
}

# View NFT metadata
POST /api/nft/view
{
  "utxo": "txid:vout"
}
```

## Development

bash
# Run tests
make test

# Build contract
make contract

# Clean build artifacts
make clean
```

## Example result

```
## nft 0 sessions
cargo run -- view --utxo 95ba0ec753501d3378e10f1516e161d8021e09b7b47a5c06470755224a10d812

## nft 1 session
charms tx show-spell --tx $(bitcoin-cli -testnet4 getrawtransaction 6c513b09b4401acd9cc9c0da6f9f2d2b0e82fbadd786bfda9a9041454824bb07) --mock
```