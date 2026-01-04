version: '3.8'

services:
  bitcoin:
    build:
      context: ./docker/bitcoin
      dockerfile: Dockerfile
    container_name: habit-tracker-bitcoin
    ports:
      - "18443:18443"  # RPC
      - "18444:18444"  # P2P
      - "28332:28332"  # ZMQ
    volumes:
      - bitcoin_data:/data
    networks:
      - habit-tracker-net
    healthcheck:
      test: ["CMD", "bitcoin-cli", "-regtest", "-rpcuser=test", "-rpcpassword=test321", "getblockchaininfo"]
      interval: 10s
      timeout: 5s
      retries: 5

  mempool-db:
    image: mariadb:10.11
    container_name: habit-tracker-mempool-db
    environment:
      MYSQL_DATABASE: mempool
      MYSQL_USER: mempool
      MYSQL_PASSWORD: mempool
      MYSQL_ROOT_PASSWORD: admin
    volumes:
      - mempool_db:/var/lib/mysql
    networks:
      - habit-tracker-net
    healthcheck:
      test: ["CMD", "mysqladmin", "ping", "-h", "localhost"]
      interval: 10s
      timeout: 5s
      retries: 5

  mempool-api:
    image: mempool/backend:latest
    container_name: habit-tracker-mempool-api
    depends_on:
      bitcoin:
        condition: service_healthy
      mempool-db:
        condition: service_healthy
    environment:
      MEMPOOL_BACKEND: "none"
      CORE_RPC_HOST: "bitcoin"
      CORE_RPC_PORT: "18443"
      CORE_RPC_USERNAME: "test"
      CORE_RPC_PASSWORD: "test321"
      DATABASE_ENABLED: "true"
      DATABASE_HOST: "mempool-db"
      DATABASE_DATABASE: "mempool"
      DATABASE_USERNAME: "mempool"
      DATABASE_PASSWORD: "mempool"
      STATISTICS_ENABLED: "true"
    volumes:
      - ./docker/mempool/backend-config.json:/backend/mempool-config.json:ro
    networks:
      - habit-tracker-net
    restart: unless-stopped

  mempool-web:
    image: mempool/frontend:latest
    container_name: habit-tracker-mempool-web
    depends_on:
      - mempool-api
    ports:
      - "8080:8080"
    environment:
      FRONTEND_HTTP_PORT: "8080"
      BACKEND_MAINNET_HTTP_HOST: "mempool-api"
    networks:
      - habit-tracker-net
    restart: unless-stopped

networks:
  habit-tracker-net:
    driver: bridge

volumes:
  bitcoin_data:
  mempool_db: