#!/bin/bash
set -e

COLOR='\033[0;36m'
NO_COLOR='\033[0m'

DATADIR="/data"

# Create bitcoin.conf
create_config() {
  echo -e "${COLOR}Creating Bitcoin configuration...${NO_COLOR}"
  mkdir -p $DATADIR
  cat <<EOF >$DATADIR/bitcoin.conf
regtest=1
server=1
txindex=1
fallbackfee=0.0001
mempoolfullrbf=1

[regtest]
rpcuser=test
rpcpassword=test321
rpcbind=0.0.0.0
rpcallowip=0.0.0.0/0
rpcport=18443

# ZMQ for mempool
zmqpubrawblock=tcp://0.0.0.0:28332
zmqpubrawtx=tcp://0.0.0.0:28332
zmqpubhashtx=tcp://0.0.0.0:28332
zmqpubhashblock=tcp://0.0.0.0:28332
EOF
}

# Start bitcoind
start_node() {
  echo -e "${COLOR}Starting Bitcoin Core (regtest)...${NO_COLOR}"
  bitcoind -datadir=$DATADIR -printtoconsole &
  BITCOIN_PID=$!
  
  # Wait for RPC to be ready
  echo -e "${COLOR}Waiting for Bitcoin RPC...${NO_COLOR}"
  until bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 -rpcwait getblockchaininfo > /dev/null 2>&1; do
    sleep 1
  done
  echo -e "${COLOR}✓ Bitcoin RPC ready${NO_COLOR}"
}

# Create wallets
create_wallets() {
  echo -e "${COLOR}Creating wallets...${NO_COLOR}"
  
  # Create descriptor wallet for the app
  bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
    -named createwallet wallet_name="test" descriptors=true > /dev/null
  
  # Create miner wallet
  bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
    -named createwallet wallet_name="miner" descriptors=false > /dev/null
  
  echo -e "${COLOR}✓ Wallets created${NO_COLOR}"
}

# Mine initial blocks
mine_initial_blocks() {
  echo -e "${COLOR}Mining initial blocks...${NO_COLOR}"
  
  MINER_ADDR=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
    -rpcwallet=miner getnewaddress "Mining Rewards")
  
  # Mine 101 blocks (coinbase maturity)
  bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
    -rpcwallet=miner generatetoaddress 101 $MINER_ADDR > /dev/null
  
  echo -e "${COLOR}✓ Mined 101 blocks${NO_COLOR}"
}

# Fund test wallet
fund_test_wallet() {
  echo -e "${COLOR}Funding test wallet...${NO_COLOR}"
  
  TEST_ADDR=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
    -rpcwallet=test getnewaddress)
  
  # Send 50 BTC to test wallet
  bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
    -rpcwallet=miner sendtoaddress $TEST_ADDR 50 > /dev/null
  
  # Mine 1 block to confirm
  MINER_ADDR=$(bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
    -rpcwallet=miner getnewaddress)
  bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
    -rpcwallet=miner generatetoaddress 1 $MINER_ADDR > /dev/null
  
  echo -e "${COLOR}✓ Test wallet funded with 50 BTC${NO_COLOR}"
}

# Cleanup on exit
cleanup() {
  echo -e "${COLOR}Shutting down Bitcoin Core...${NO_COLOR}"
  bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 stop || true
  wait $BITCOIN_PID 2>/dev/null || true
}

trap cleanup EXIT INT TERM

# Main execution
echo -e "${COLOR}================================${NO_COLOR}"
echo -e "${COLOR}Habit Tracker - Bitcoin Regtest${NO_COLOR}"
echo -e "${COLOR}================================${NO_COLOR}"

create_config
start_node
create_wallets
mine_initial_blocks
fund_test_wallet

echo -e "${COLOR}================================${NO_COLOR}"
echo -e "${COLOR}✓ Setup complete!${NO_COLOR}"
echo -e "${COLOR}================================${NO_COLOR}"
echo -e "${COLOR}RPC: http://localhost:18443${NO_COLOR}"
echo -e "${COLOR}User: test${NO_COLOR}"
echo -e "${COLOR}Pass: test321${NO_COLOR}"
echo -e "${COLOR}Mempool: http://localhost:8080${NO_COLOR}"
echo -e "${COLOR}================================${NO_COLOR}"

# Keep container running and follow logs
wait $BITCOIN_PID