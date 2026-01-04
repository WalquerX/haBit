#!/bin/sh
set -e

echo "🔧 Initializing Bitcoin regtest environment..."

# Wait for Bitcoin RPC
echo "⏳ Waiting for Bitcoin RPC..."
until bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin getblockchaininfo > /dev/null 2>&1; do
  sleep 1
done
echo "✓ Bitcoin RPC ready"

# Check if wallets are loaded, load them if not
echo "📁 Checking wallets..."

if ! bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin listwallets | grep -q "test"; then
  if ! bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin loadwallet test 2>/dev/null; then
    bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
      -named createwallet wallet_name="test" descriptors=true > /dev/null
    echo "✓ Created 'test' wallet (descriptor, Taproot-enabled)"
  else
    echo "✓ Loaded existing 'test' wallet"
  fi
else
  echo "✓ Wallet 'test' already loaded"
fi

if ! bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin listwallets | grep -q "miner"; then
  if ! bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin loadwallet miner 2>/dev/null; then
    bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
      -named createwallet wallet_name="miner" descriptors=true > /dev/null
    echo "✓ Created 'miner' wallet (descriptor)"
  else
    echo "✓ Loaded existing 'miner' wallet"
  fi
else
  echo "✓ Wallet 'miner' already loaded"
fi

# Check block count
BLOCK_COUNT=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin getblockcount)

if [ "$BLOCK_COUNT" -lt 110 ]; then
  echo "⛏️  Mining initial blocks..."
  
  MINER_ADDR=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
    -rpcwallet=miner getnewaddress "Mining Rewards")
  
  BLOCKS_TO_MINE=$((110 - BLOCK_COUNT))
  bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
    -rpcwallet=miner generatetoaddress "$BLOCKS_TO_MINE" "$MINER_ADDR" > /dev/null
  
  echo "✓ Mined $BLOCKS_TO_MINE blocks (total: 110)"
else
  echo "✓ Chain has $BLOCK_COUNT blocks"
fi

# Check test wallet balance - fund if empty or low
TEST_BALANCE=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
  -rpcwallet=test getbalance 2>/dev/null || echo "0")

# Simple check: if balance is less than 10 BTC, fund it
# Using awk instead of bc for floating point comparison
NEEDS_FUNDING=$(echo "$TEST_BALANCE" | awk '{if ($1 < 10) print "yes"; else print "no"}')

if [ "$NEEDS_FUNDING" = "yes" ]; then
  echo "💰 Funding test wallet (current balance: $TEST_BALANCE BTC)..."
  
  # Get fresh address
  TEST_ADDR=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
    -rpcwallet=test getnewaddress)
  
  # Send 50 BTC
  TXID=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
    -rpcwallet=miner sendtoaddress "$TEST_ADDR" 50 2>/dev/null || echo "")
  
  if [ -n "$TXID" ]; then
    echo "✓ Sent 50 BTC to test wallet"
    
    # Mine 1 block to confirm
    MINER_ADDR=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
      -rpcwallet=miner getnewaddress)
    bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin \
      -rpcwallet=miner generatetoaddress 1 "$MINER_ADDR" > /dev/null
    
    echo "✓ Transaction confirmed"
  else
    echo "⚠️  Could not send funds"
  fi
else
  echo "✓ Test wallet has sufficient funds ($TEST_BALANCE BTC)"
fi

# Final status
FINAL_BLOCKS=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin getblockcount)
TEST_FINAL=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin -rpcwallet=test getbalance)
MINER_FINAL=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcconnect=bitcoin -rpcwallet=miner getbalance)

echo ""
echo "=========================================="
echo "✅ Bitcoin regtest ready!"
echo "=========================================="
echo "Bitcoin Core: 27.1"
echo "Blockchain: $FINAL_BLOCKS blocks"
echo "Test wallet: $TEST_FINAL BTC"
echo "Miner wallet: $MINER_FINAL BTC"
echo ""
echo "RPC: http://localhost:18443"
echo "User: test / Pass: test321"
echo "Mempool: http://localhost:8080"
echo "=========================================="
echo ""
echo "Ready for testing! Try:"
echo "  export USE_DOCKER=1"
echo "  cargo run -- create --habit 'Morning Run'"
echo "=========================================="