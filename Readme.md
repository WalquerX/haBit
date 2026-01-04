# ⚔️ HaBit Tracker

A Bitcoin NFT-based habit tracker built with [Charms Protocol](https://github.com/CharmsDev/charms), inspired by the Way of the Warrior and the science of habit formation.

## 🎯 The Philosophy

### The Way of the Warrior

It takes **66 days** to wire a new habit into your brain's neural pathways. We've combined this science with samurai philosophy to create a journey of transformation.

**Three Stages of Transformation:**

1. **🔴 DESTRUCTION (Days 1-22)**: Breaking old patterns
   - *"Destroying the old self to make room for the new"*
   - Like forging a blade, the metal must first be broken down

2. **🟡 INSTALLATION (Days 23-44)**: Forging the new way
   - *"Heat, hammer, repeat. The blade takes shape."*
   - Neural pathways forming, habit becoming stronger

3. **🟢 INTEGRATION (Days 45-66)**: Becoming the master
   - *"The habit becomes part of you, like breathing"*
   - Automaticity achieved, you are now the master

## ✨ Features

- **On-Chain Habit Tracking** - Verifiable proof of discipline on Bitcoin
- **66-Day Journey** - Science-backed path to true mastery
- **Samurai Badge System** - 25 achievement badges inspired by warrior culture
- **Three Transformation Stages** - Track your progress through Destruction, Installation, Integration
- **Legendary Tier** - Beyond mastery (100, 200, 365, 500, 1000 days)
- **CLI & API** - Command-line interface and HTTP server
- **Local Development** - Docker environment with Mempool explorer

> **Note:** This is a hackathon/demo project. The smart contract is currently in testing mode.

## 🚀 Quick Start

### Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose
- [Rust](https://rustup.rs/) (latest stable)
- [Charms CLI](https://github.com/CharmsDev/charms) for building contracts

### 1. Build the Contract
```bash
make contract
```

### 2. Start Development Environment
```bash
# Start Bitcoin regtest + Mempool explorer
./docker-helper.sh start

# Set environment variables
export USE_DOCKER=1
export RUST_LOG=info
```

### 3. Begin Your Journey
```bash
# Create your habit tracker
cargo run -- create --habit "Morning Meditation"

# Mine a block to confirm
./docker-helper.sh mine

# View your progress
cargo run -- view --utxo <txid>:0
```

Output:
```
⚔️  SAMURAI HABIT TRACKER
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Habit: Morning Meditation
   Sessions: 0/66
   Stage: 🔴 Stage 1: DESTRUCTION
   Progress: [░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 0%

   Begin your journey, warrior.
   🌸 Complete your first session to earn 'First Blood'
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 4. Complete Your First Session
```bash
# Update (complete a session)
cargo run -- update --utxo <txid>:0

# Mine to confirm
./docker-helper.sh mine

# View your progress
cargo run -- view --utxo <new-txid>:0
```

Output:
```
⚔️  SESSION COMPLETE
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   Habit: Morning Meditation
   Sessions: 0 → 1/66
   Stage: DESTRUCTION

🏆 NEW BADGE UNLOCKED!
   🌸 First Blood
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

## 📋 CLI Commands

### Create a Habit
```bash
cargo run -- create --habit "Your Habit Name"
```

### Complete a Session
```bash
cargo run -- update --utxo <txid>:<vout>
```

### View Progress
```bash
cargo run -- view --utxo <txid>:<vout>
```

Shows:
- Current stage (Destruction, Installation, Integration, or Legendary)
- Progress bar toward 66-day mastery
- All earned badges
- Next milestone

## 🏆 The Badge System

### Stage 1: DESTRUCTION (Days 1-22) - Breaking Old Patterns

| Session | Badge | Meaning |
|---------|-------|---------|
| 1 | 🌸 **First Blood** | *"The journey of a thousand ri begins with a single step"* |
| 3 | ⚔️ **Three Cuts** | *"The blade that cuts through hesitation"* |
| 7 | 🔥 **Week Warrior** | *"Seven days of unbroken discipline"* |
| 11 | 🌊 **Rising Tide** | *"The wave builds momentum"* |
| 15 | ⛩️ **Temple Guardian** | *"Halfway through the destruction stage"* |
| 22 | 💥 **Destruction Complete** | *"Old patterns shattered. The warrior emerges."* |

### Stage 2: INSTALLATION (Days 23-44) - Forging the New Way

| Session | Badge | Meaning |
|---------|-------|---------|
| 23 | 🔨 **The Forge Begins** | *"Heat, hammer, repeat. The blade takes shape."* |
| 30 | 🗡️ **Month of Steel** | *"30 sessions. Neural pathways forming."* |
| 33 | ⚡ **Thunder Strike** | *"Power builds in the installation phase"* |
| 40 | 🌙 **Moonlit Path** | *"The way becomes clearer"* |
| 44 | 🎌 **Installation Complete** | *"The new way is forged. Not yet automatic, but strong."* |

### Stage 3: INTEGRATION (Days 45-66) - Becoming the Master

| Session | Badge | Meaning |
|---------|-------|---------|
| 45 | 🌅 **Dawn of Mastery** | *"The final stage begins. Automaticity awaits."* |
| 50 | 🏔️ **Mountain Summit** | *"50 sessions. The path is almost yours."* |
| 55 | 🐉 **Dragon Awakens** | *"The habit becomes part of you"* |
| 60 | ⭐ **Celestial Alignment** | *"Six sessions from mastery"* |
| 66 | 👑 **Shogun** | *"66 days complete. True mastery achieved. The habit is YOU."* |

### Beyond Mastery (Legendary Tier)

| Session | Badge | Meaning |
|---------|-------|---------|
| 100 | 💯 **Century Samurai** | *"One hundred sessions of unwavering dedication"* |
| 200 | 🌸⚔️ **Twin Blades** | *"Miyamoto Musashi level dedication"* |
| 365 | 🏯 **Daimyo** | *"A full year. You are the lord of your domain."* |
| 500 | 🔮 **Mystic Warrior** | *"Beyond mortal discipline"* |
| 1000 | ⛩️👑 **Living Legend** | *"Your name will be spoken for generations"* |

## 🐳 Docker Helper Commands
```bash
./docker-helper.sh status    # Check system status
./docker-helper.sh mine [n]  # Mine n blocks
./docker-helper.sh fund      # Add 50 BTC to test wallet
./docker-helper.sh logs      # View Bitcoin logs
./docker-helper.sh restart   # Restart services
./docker-helper.sh reset     # Reset everything
```

## 🌐 Services

- **Bitcoin RPC**: `http://localhost:18443` (user: `test`, pass: `test321`)
- **Mempool Explorer**: `http://localhost:8080`

## 🔍 Viewing Transactions in Mempool Explorer

Your local Mempool explorer runs at `http://localhost:8080` when Docker is running.

### How to Use

1. **Open the explorer**: Visit `http://localhost:8080` in your browser
2. **Search for your transactions**: Paste any transaction ID (txid) from the CLI output
3. **View on-chain data**: See NFT metadata, inputs, outputs, and full transaction details

### Example

After creating an NFT:
```bash
cargo run -- create --habit "Daily Exercise"
# Output: UTXO: fb964e566c47d78f9278b9d962e5889f09d5b77736b822cb09740918be43c17f:0
```

Search for the txid in Mempool explorer to see your NFT creation transaction with all the on-chain data!

## 🧪 Development

### Running Tests
```bash
cargo test
```

### Project Structure
```
habit-tracker/
├── contracts/           # Smart contract (badge validation)
├── src/
│   ├── main.rs         # CLI and API server
│   ├── nft.rs          # NFT operations
│   └── tests.rs        # Integration tests
├── docker/             # Development environment
└── docker-compose.yml  # Services
```

### Building
```bash
cargo build              # Build application
cargo build --release    # Build in release mode
make contract           # Build contract
make clean              # Clean build artifacts
```

## 🔌 API Server (Experimental)
```bash
cargo run  # Starts server on http://127.0.0.1:3000
```

Endpoints:
- `POST /api/nft/create/unsigned` - Create habit
- `POST /api/nft/update/unsigned` - Complete session
- `POST /api/nft/broadcast` - Broadcast signed transactions
- `POST /api/nft/view` - View habit details

## 🚀 Roadmap & Future Development

### Planned Features

#### 🔒 Commitment Staking
Lock funds as commitment. Break the streak, lose the stake.

**Features:**
- Stake Bitcoin when creating a habit
- Set custom commitment periods (7, 30, 90 days)
- Automatic refund for maintained streaks
- Integration with Scrolls API for programmable conditions
- Difficulty-based stake amounts

**Use Case:**
```bash
cargo run -- create \
  --habit "Gym 5x/week" \
  --stake 1000000 \
  --period 66
```

#### 🤝 Verified Habits (Multi-Signature)
Require third-party verification (gym, coach, teacher).

**Features:**
- **2-of-2 multisig**: Both you and verifier must sign
- **Trusted verifiers**: Gyms, teachers, friends, coaches
- **Penalty mechanism**: Failed commitments → funds to charity/non-profit
- **Verifier network**: Public registry of trusted validators

**Use Case:**
```bash
cargo run -- create \
  --habit "Daily Training" \
  --verifier "gym_pubkey" \
  --stake 500000 \
  --penalty-address "bc1q...nonprofit..."
```

#### 💀 Habit-Sustained NFT (Decaying Mechanism)

**Concept**: Your NFT's vitality is tied to habit completion. Miss habits → NFT degrades and eventually "dies".

**Core Mechanic:**
- NFT has a "health meter" stored in its metadata (0-100%)
- Each missed habit day reduces health by X%
- At 0% health, NFT "dies" (burns, transforms to grayscale, or becomes non-transferable)
- Completing streaks restores/boosts health
- Monthly circulation requirement waived if habits are maintained

**Anti-Hoarding Mechanism:**
Traditional decaying NFTs require monthly transfers to stay alive. Habit-sustained NFTs flip this:
- **Instead of**: Transfer monthly or NFT dies
- **Now**: Complete habits or NFT dies
- Keeps habits alive through discipline, not just circulation
- NFT becomes a living representation of your commitment

**Implementation:**
```bash
# Create habit with decay enabled
cargo run -- create \
  --habit "Morning Run" \
  --enable-decay \
  --decay-rate 5  # Lose 5% health per missed day

# View health status
cargo run -- view --utxo <txid>:0
# Output: Health: 85% (missed 3 days)

# Complete session to restore health
cargo run -- update --utxo <txid>:0
# Output: Health: 95% (+10% restored)
```

**Use Cases:**
- **Accountability NFTs**: Visual proof of consistent effort
- **Dynamic Art**: NFT appearance changes based on health (full color → grayscale)
- **Commitment Proof**: A degraded NFT shows broken discipline
- **Resurrection Mechanism**: Can "revive" dead NFTs with intensive streak (e.g., 7 consecutive days)

**Visual States:**
- **100% Health**: Full vibrant samurai artwork
- **75% Health**: Colors begin to fade
- **50% Health**: Partial grayscale, cracks appear
- **25% Health**: Mostly grayscale, significant degradation
- **0% Health**: Complete grayscale, marked as "Fallen Warrior"

This creates a powerful psychological incentive: your NFT literally reflects your commitment. Let it die, and everyone can see you broke discipline. Keep it alive, and it's permanent proof of your warrior spirit.

#### 📊 Social Features
- **Leaderboards**: Compete on longest streaks
- **Habit challenges**: Group commitments with shared stakes
- **NFT marketplace**: Trade rare badges (1000-day streaks)
- **Progress sharing**: Export proof to social media

### Integration Ideas

- **Fitness Apps**: Auto-verify via Strava/Apple Health
- **Educational Platforms**: Teacher-verified study sessions
- **Wellness Providers**: Therapist-confirmed sessions
- **Non-Profit Partnerships**: Failed stakes → verified charities

### Technical Roadmap

1. **Phase 1** (Current): Basic NFT, session tracking, 66-day badge system
2. **Phase 2**: Commitment staking with Scrolls integration
3. **Phase 3**: Multi-signature verification system
4. **Phase 4**: Habit-sustained NFT decay mechanism
5. **Phase 5**: Social features and leaderboards
6. **Phase 6**: Third-party integrations and API ecosystem

## 🔗 Complementary Repo

- **Frontend**: [habitChain](https://github.com/DuwalVC/habitChain) - Web interface

---

*"The blade that cuts through resistance is forged one session at a time."* ⚔️