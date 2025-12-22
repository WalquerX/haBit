.PHONY: all contract backend run test clean

# Build everything
all: contract backend

# Build contract only
contract:
	@echo "🔨 Building contract..."
	@cd contract && cargo build --release --target wasm32-wasip1
	@mkdir -p contracts
	@echo "📦 Processing contract with charms..."
	@cd contract && \
		APP_BIN=$$(charms app build) && \
		echo "   Built app binary: $$APP_BIN" && \
		VK=$$(charms app vk "$$APP_BIN") && \
		echo "   Got VK: $$VK" && \
		cp "$$APP_BIN" ../contracts/habit-tracker.wasm && \
		echo "$$VK" > ../contracts/habit-tracker.vk
	@echo "✅ Contract ready"
	@echo "   VK: $$(cat contracts/habit-tracker.vk)"

# Build backend only
backend:
	@echo "🔨 Building backend..."
	@cargo build --release
	@echo "✅ Backend ready"

# Run server
run: contract
	@cargo run

# Run tests
test: contract
	@cargo test -- --test-threads=1

# Clean everything
clean:
	@cargo clean
	@cd contract && cargo clean
	@rm -rf contracts

# Show available commands
help:
	@echo "Available commands:"
	@echo "  make all      - Build contract and backend"
	@echo "  make contract - Build only the contract"
	@echo "  make backend  - Build only the backend"
	@echo "  make run      - Build and run server"
	@echo "  make test     - Build and run tests"
	@echo "  make clean    - Clean all build artifacts"
	@echo "  make help     - Show this help"
