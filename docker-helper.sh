#!/bin/bash

case "$1" in
  start)
    echo "🚀 Starting Habit Tracker development environment..."
    docker compose up -d
    echo ""
    echo "⏳ Waiting for initialization..."
    sleep 5
    docker compose logs bitcoin-init
    echo ""
    echo "✅ Ready! Set environment:"
    echo "   export USE_DOCKER=1"
    echo "   export RUST_LOG=info"
    ;;
    
  stop)
    echo "🛑 Stopping services..."
    docker compose down
    ;;
    
  restart)
    echo "🔄 Restarting services..."
    docker compose restart
    docker compose logs -f bitcoin-init
    ;;
    
  reset)
    echo "⚠️  Resetting all data..."
    read -p "Are you sure? This will delete all blockchain data and wallets. (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
      docker compose down -v
      echo "✅ Reset complete. Run './docker-helper.sh start' to begin fresh"
    fi
    ;;
    
  logs)
    docker compose logs -f bitcoin
    ;;
    
  init)
    echo "🔧 Re-running initialization..."
    docker compose rm -f bitcoin-init
    docker compose up -d bitcoin-init
    docker compose logs -f bitcoin-init
    ;;
    
  fund)
    echo "💰 Funding test wallet with 50 BTC..."
    TEST_ADDR=$(docker compose exec bitcoin bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
      -rpcwallet=test getnewaddress | tr -d '\r')
    echo "   Address: $TEST_ADDR"
    
    docker compose exec bitcoin bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
      -rpcwallet=miner sendtoaddress "$TEST_ADDR" 50
    
    docker compose exec bitcoin bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
      -rpcwallet=miner -generate 1
    
    BALANCE=$(docker compose exec bitcoin bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
      -rpcwallet=test getbalance)
    echo "   Balance: $BALANCE BTC"
    ;;
    
  mine)
    BLOCKS=${2:-1}
    echo "⛏️  Mining $BLOCKS block(s)..."
    docker compose exec bitcoin bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
      -rpcwallet=miner -generate "$BLOCKS"
    ;;
    
  status)
    echo "📊 Habit Tracker Status"
    echo "======================="
    
    BLOCKS=$(docker compose exec bitcoin bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
      getblockcount 2>/dev/null | tr -d '\r' || echo "N/A")
    TEST_BAL=$(docker compose exec bitcoin bitcoin-cli -regtest -rpcuser=test -rpcpassword=test321 \
      -rpcwallet=test getbalance 2>/dev/null | tr -d '\r' || echo "N/A")
    
    echo "Blocks: $BLOCKS"
    echo "Test wallet: $TEST_BAL BTC"
    echo ""
    echo "Services:"
    docker compose ps
    ;;
    
  *)
    echo "Habit Tracker Docker Helper"
    echo ""
    echo "Usage: ./docker-helper.sh [command]"
    echo ""
    echo "Commands:"
    echo "  start    - Start all services"
    echo "  stop     - Stop all services"
    echo "  restart  - Restart services"
    echo "  reset    - Reset all data (WARNING: deletes everything)"
    echo "  logs     - Show Bitcoin logs"
    echo "  init     - Re-run initialization"
    echo "  fund     - Add 50 BTC to test wallet"
    echo "  mine [n] - Mine n blocks (default: 1)"
    echo "  status   - Show current status"
    ;;
esac