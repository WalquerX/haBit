//! NFT operations for the Habit Tracker
//!
//! This module handles all NFT-related operations including creation, updates,
//! and metadata extraction using the Charms protocol.
use base64::Engine;
use bitcoincore_rpc::bitcoin;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use charms_client::tx::Tx;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::process::Command;
use std::str::FromStr;
use tempfile::NamedTempFile;

// ============================================================================
// Constants
// ============================================================================

/// NFT UTXO value in satoshis (1000 sats = 0.00001 BTC)
const NFT_AMOUNT_SATS: u64 = 1000;

/// Minimum funding required for operations (covers NFT + fees)
const MIN_FUNDING_SATS: u64 = 2000;

/// Default fee rate for transactions (sats/vB)
const DEFAULT_FEE_RATE: f64 = 2.0;

/// Badge milestones - The Samurai Path to Mastery (66 Days)
const BADGE_MILESTONES: &[(u64, &str)] = &[
    // Stage 1: DESTRUCTION (Days 1-22) - Breaking Old Patterns
    (1, "🌸 First Blood"),
    (3, "⚔️ Three Cuts"),
    (7, "🔥 Week Warrior"),
    (11, "🌊 Rising Tide"),
    (15, "⛩️ Temple Guardian"),
    (22, "💥 Destruction Complete"),
    // Stage 2: INSTALLATION (Days 23-44) - Forging the New Way
    (23, "🔨 The Forge Begins"),
    (30, "🗡️ Month of Steel"),
    (33, "⚡ Thunder Strike"),
    (40, "🌙 Moonlit Path"),
    (44, "🎌 Installation Complete"),
    // Stage 3: INTEGRATION (Days 45-66) - Becoming the Master
    (45, "🌅 Dawn of Mastery"),
    (50, "🏔️ Mountain Summit"),
    (55, "🐉 Dragon Awakens"),
    (60, "⭐ Celestial Alignment"),
    (66, "👑 Shogun"),
    // Beyond Mastery (Legendary Tier)
    (100, "💯 Century Samurai"),
    (200, "🌸⚔️ Twin Blades"),
    (365, "🏯 Daimyo"),
    (500, "🔮 Mystic Warrior"),
    (1000, "⛩️👑 Living Legend"),
];

// ============================================================================
// Public Response Types
// ============================================================================

#[derive(Serialize)]
pub struct UnsignedNftResponse {
    pub commit_tx_hex: String,
    pub spell_tx_hex: String,
    pub commit_txid: String, // For reference
    pub spell_inputs_info: Vec<SigningInputInfo>,
}

#[derive(Serialize, Debug)]
pub struct UnsignedUpdateResponse {
    pub commit_tx_hex: String,
    pub spell_tx_hex: String,
    pub commit_txid: String,
    pub spell_inputs_info: Vec<SigningInputInfo>,
    pub current_sessions: u64,
    pub new_sessions: u64,
}

#[derive(Serialize, Debug)]
pub struct SigningInputInfo {
    pub tx_index: usize,    // 0 = commit, 1 = spell
    pub input_index: usize, // Which input in the tx
    pub prev_script_hex: String,
    pub amount_sats: u64,
}

#[derive(Serialize)]
pub struct BroadcastNftResponse {
    pub commit_txid: String,
    pub spell_txid: String,
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get badges for a given session count
fn get_badges_for_sessions(sessions: u64) -> Vec<String> {
    BADGE_MILESTONES
        .iter()
        .filter(|(threshold, _)| sessions >= *threshold)
        .map(|(_, badge)| badge.to_string())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProverBackend {
    _Http,
    CliMock,
}

impl ProverBackend {
    pub fn _auto_detect(btc: &Client) -> anyhow::Result<Self> {
        let info = btc.get_blockchain_info()?;
        match info.chain {
            bitcoincore_rpc::bitcoin::Network::Regtest => {
                println!("Detected regtest - using CLI mock mode");
                Ok(ProverBackend::CliMock)
            }
            _ => {
                println!("Detected {} - using HTTP API", info.chain);
                Ok(ProverBackend::_Http)
            }
        }
    }
}

/// Get the path to the compiled contract WASM
pub fn get_contract_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/habit-tracker.wasm")
}

/// Get the path to the contract verification key
pub fn get_contract_vk_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/habit-tracker.vk")
}

/// Load contract WASM and verification key
pub fn load_contract() -> anyhow::Result<(String, String)> {
    let contract_path = get_contract_path();
    if !contract_path.exists() {
        anyhow::bail!(
            "Contract WASM not found at {:?}\n\
             Build it with: make contract",
            contract_path
        );
    }

    // Load VK from file
    let vk_path = get_contract_vk_path();
    let vk = if vk_path.exists() {
        fs::read_to_string(&vk_path)?.trim().to_string()
    } else {
        anyhow::bail!(
            "Contract VK not found at {:?}\n\
             Build it with: make contract",
            vk_path
        );
    };

    let binary_bytes = fs::read(&contract_path)?;
    let binary_base64 = base64::engine::general_purpose::STANDARD.encode(&binary_bytes);

    log::debug!("Loaded contract from {:?}", contract_path);
    Ok((vk, binary_base64))
}

/// Connect to Bitcoin Core RPC
pub fn connect_bitcoin() -> anyhow::Result<Client> {
    let (url, auth) = if std::env::var("USE_DOCKER").is_ok() {
        // Docker regtest - must specify wallet in URL path
        log::debug!("Using Docker Bitcoin regtest");
        (
            "http://127.0.0.1:18443/wallet/test".to_string(), // Added /wallet/test
            Auth::UserPass("test".to_string(), "test321".to_string()),
        )
    } else {
        // Default: testnet4 with cookie
        let cookie_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".bitcoin/testnet4/.cookie");

        log::debug!("Using testnet4 node");
        (
            "http://127.0.0.1:48332/wallet/test".to_string(),
            Auth::CookieFile(cookie_path),
        )
    };

    let btc = Client::new(&url, auth)?;
    log::info!("Connected to Bitcoin Core RPC at {}", url);
    Ok(btc)
}
// pub fn connect_bitcoin() -> anyhow::Result<Client> {
//     let cookie_path = dirs::home_dir()
//         .ok_or_else(|| anyhow::anyhow!("No home dir"))?
//         .join(".bitcoin/testnet4/.cookie");

//     let btc = Client::new(
//         "http://127.0.0.1:48332/wallet/test",
//         Auth::CookieFile(cookie_path),
//     )?;

//     log::debug!("Connected to Bitcoin Core RPC");
//     Ok(btc)
// }

/// Get a suitable funding UTXO, excluding specified UTXOs
pub fn get_funding_utxo(
    btc: &Client,
    exclude_utxo: Option<&str>,
) -> anyhow::Result<(String, u64, String)> {
    let utxos = btc.list_unspent(None, None, None, None, None)?;
    let network = btc.get_blockchain_info()?.chain;

    let funding = utxos.iter().find(|utxo| {
        let utxo_id = format!("{}:{}", utxo.txid, utxo.vout);
        let is_nft = utxo.amount.to_sat() == 1000;
        let is_excluded = exclude_utxo.is_some_and(|excluded| utxo_id == excluded);
        !is_nft && !is_excluded
    });

    if let Some(funding) = funding {
        let addr = funding
            .address
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Funding UTXO has no address"))?
            .clone()
            .require_network(network)?
            .to_string();

        log::debug!("Found funding UTXO: {}:{}", funding.txid, funding.vout);
        Ok((
            format!("{}:{}", funding.txid, funding.vout),
            funding.amount.to_sat(),
            addr,
        ))
    } else {
        let new_addr = btc
            .get_new_address(None, None)?
            .require_network(network)?
            .to_string();

        anyhow::bail!(
            "No funding UTXOs available. Fund this address:\n   {}\n\nNetwork: {:?}",
            new_addr,
            network
        );
    }
}

/// Generate a unique app ID for this spell
fn generate_app_id(vk: &str) -> String {
    let identity_input = format!("habit_tracker_{}", chrono::Utc::now().timestamp());
    let mut hasher = Sha256::new();
    hasher.update(identity_input.as_bytes());
    let identity_hash = hasher.finalize();
    let identity_hex = hex::encode(identity_hash);
    format!("n/{}/{}", identity_hex, vk)
}

// ============================================================================
// NFT Metadata Operations
// ============================================================================

pub fn extract_nft_metadata(btc: &Client, txid: &str) -> anyhow::Result<(String, u64, String)> {
    log::debug!("Extracting NFT metadata from {}", txid);

    let tx_hex = btc.get_raw_transaction_hex(&bitcoin::Txid::from_str(txid)?, None)?;

    let spell_output = Command::new("charms")
        .args(["tx", "show-spell", "--tx", &tx_hex, "--mock", "--json"])
        .output()?;

    if !spell_output.status.success() {
        anyhow::bail!("Failed to extract spell");
    }

    let spell: serde_json::Value = serde_json::from_slice(&spell_output.stdout)?;

    let charms = spell
        .get("outs")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|out| out.get("charms"))
        .and_then(|c| c.get("$0000"))
        .ok_or_else(|| anyhow::anyhow!("No charms found in spell"))?;

    let habit_name = charms
        .get("habit_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Meditation")
        .to_string();

    let sessions = charms
        .get("total_sessions")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let owner = charms
        .get("owner")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No owner found in NFT"))?
        .to_string();

    log::info!(
        "NFT metadata - Habit: '{}', Sessions: {}, Owner: {}",
        habit_name,
        sessions,
        &owner[..12]
    );

    Ok((habit_name, sessions, owner))
}

// ============================================================================
// Prover Integration
// ============================================================================


use std::path::PathBuf;
use std::env;

fn find_charms_binary() -> anyhow::Result<PathBuf> {
    // 1. Check environment variable first (highest priority)
    if let Ok(custom_path) = env::var("CHARMS_BIN") {
        let path = PathBuf::from(custom_path);
        if path.exists() {
            return Ok(path);
        }
        anyhow::bail!("CHARMS_BIN set to {:?} but binary not found", path);
    }

    // 2. Check if charms is in PATH
    if let Ok(output) = Command::new("which").arg("charms").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    // 3. Fall back to local dev path
    if let Some(home) = dirs::home_dir() {
        let local_path = home.join("BOS/charms/target/release/charms");
        if local_path.exists() {
            return Ok(local_path);
        }
    }

    anyhow::bail!(
        "charms binary not found. Try one of:\n\
         - Set CHARMS_BIN=/path/to/charms\n\
         - Add charms to your PATH\n\
         - Build locally: cd ~/BOS/charms && cargo build --release"
    )
}

pub fn prove_with_cli(
    spell: &serde_json::Value,
    contract_path: &str,
    prev_txs: &[String],
    funding_utxo: &str,
    funding_utxo_value: u64,
    change_address: &str,
    fee_rate: f64,
) -> anyhow::Result<Vec<Tx>> {
    // Write spell to temporary file
    let mut spell_file = NamedTempFile::new()?;
    spell_file.write_all(serde_json::to_string_pretty(spell)?.as_bytes())?;
    let spell_path = spell_file.path().to_str().unwrap();

       // Locate charms binary - REPLACED SECTION
    let charms_bin = find_charms_binary()?;
    log::debug!("Using charms binary: {:?}", charms_bin);

    // Convert contract_path to absolute path
    let absolute_contract_path = std::fs::canonicalize(contract_path)?;
    log::debug!("Using contract: {:?}", absolute_contract_path);

    let mut cmd = Command::new(&charms_bin);
    cmd.arg("spell")
        .arg("prove")
        .arg("--spell")
        .arg(spell_path)
        .arg("--funding-utxo")
        .arg(funding_utxo)
        .arg("--funding-utxo-value")
        .arg(funding_utxo_value.to_string())
        .arg("--change-address")
        .arg(change_address)
        .arg("--fee-rate")
        .arg(fee_rate.to_string())
        .arg("--chain")
        .arg("bitcoin")
        .arg("--mock")
        .arg("--app-bins")
        .arg(absolute_contract_path);

    if !prev_txs.is_empty() {
        cmd.arg("--prev-txs").arg(prev_txs.join(","));
    }

    log::debug!("Calling prover...");
    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("charms spell prove failed: {}", stderr);
    }

    let stdout = String::from_utf8(output.stdout)?;
    let txs: Vec<Tx> = serde_json::from_str(&stdout)
        .map_err(|e| anyhow::anyhow!("Failed to parse CLI output: {}", e))?;

    log::debug!("Prover generated {} transactions", txs.len());
    Ok(txs)
}

// ============================================================================
// NFT Creation
// ============================================================================

pub fn create_nft(btc: &Client, habit_name: String) -> anyhow::Result<String> {
    println!("DEBUG: Starting create_nft for habit: '{}'", habit_name);
    log::debug!("Creating Habit Tracker NFT\n");

    println!("DEBUG: Loading contract...");
    let (vk, _binary_base64) = load_contract()?;

    println!("DEBUG: Getting funding UTXO...");
    let (funding_utxo, funding_value, addr_str) = get_funding_utxo(btc, None)?;

    println!("DEBUG: Getting funding UTXO...");
    log::debug!(
        "Using funding UTXO: {} ({} sats)",
        funding_utxo,
        funding_value
    );

    println!("DEBUG: Generating app_id...");
    let app_id = generate_app_id(&vk);
    println!("DEBUG: Generating app_id...");

    println!("DEBUG: Generating app_id...");
    let spell = json!({
        "version": 8,
        "apps": {"$00": app_id},
        "ins": [],
        "outs": [{
            "address": addr_str,
            "charms": {
                "$00": {
                    "name": "🗡️ Habit Tracker",
                    "description": format!("Tracking habit: {}", habit_name),
                    "owner": addr_str,
                    "habit_name": habit_name,
                    "total_sessions": 0,
                    "created_at": chrono::Utc::now().timestamp(),
                }
            },
            "sats": NFT_AMOUNT_SATS
        }]
    });
    println!("DEBUG: Spell created");

    log::info!("\n Calling prover...");
    println!("DEBUG: Getting contract path...");
    let contract_path = get_contract_path();
    println!("DEBUG: Getting contract path...");

    println!("DEBUG: Calling prove_with_cli...");
    let txs = prove_with_cli(
        &spell,
        contract_path.to_str().unwrap(),
        &[],
        &funding_utxo,
        funding_value,
        &addr_str,
        DEFAULT_FEE_RATE,
    )?;
    println!("DEBUG: Prover returned {} transactions", txs.len());

    log::info!(" Got transactions from prover");

    let bitcoin_txs: Vec<bitcoin::Transaction> = txs
        .iter()
        .filter_map(|tx| match tx {
            Tx::Bitcoin(btx) => Some(btx.inner().clone()),
            _ => None,
        })
        .collect();

    log::debug!(
        "   Commit tx: {} bytes",
        bitcoin::consensus::serialize(&bitcoin_txs[0]).len()
    );
    log::debug!(
        "   Spell tx: {} bytes",
        bitcoin::consensus::serialize(&bitcoin_txs[1]).len()
    );

    let result = sign_and_broadcast_create(btc, bitcoin_txs)?;

    println!("DEBUG: Extracting spell txid...");
    let spell_txid = result
        .get("tx-results")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|r| r.get("txid"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get spell txid from result"))?;
    println!("DEBUG: Extracting spell txid...");

    println!("\n⚔️  HABIT CREATED - THE PATH BEGINS");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   Habit: {}", habit_name);
    println!("   Sessions: 0/66");
    println!("   UTXO: {}:0", spell_txid);
    println!("\n   'The journey of a thousand ri begins");
    println!("    with a single step.'");
    println!("\nTo complete your first session:");
    println!("   cargo run -- update --utxo {}:0", spell_txid);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(spell_txid.to_string())
}

// pub async fn update_nft(btc: &Client, nft_utxo: String) -> anyhow::Result<()> {
//     log::debug!("Updating Habit Tracker NFT\n");

//     // let backend = ProverBackend::auto_detect(btc)?;
//     let backend = ProverBackend::CliMock;
//     let (vk, binary_base64) = load_contract()?;
//     let (funding_utxo, funding_value, addr_str) = get_funding_utxo(btc, Some(&nft_utxo))?;

//     let parts: Vec<&str> = nft_utxo.split(':').collect();
//     let prev_txid = parts[0];

//     let (habit_name, current_sessions, _) = extract_nft_metadata(btc, prev_txid)?;

//     log::debug!("\n Fetching previous transaction...");

//     let prev_tx_raw = btc.get_raw_transaction_hex(&bitcoin::Txid::from_str(prev_txid)?, None)?;

//     let identity_input = format!("habit_tracker_{}", chrono::Utc::now().timestamp());
//     let mut hasher = Sha256::new();
//     hasher.update(identity_input.as_bytes());
//     let identity_hash = hasher.finalize();
//     let identity_hex = hex::encode(identity_hash);
//     let app_id = format!("n/{}/{}", identity_hex, vk);

//     let spell = json!({
//         "version": 8,
//         "apps": {"$00": app_id},
//         "ins": [{
//             "utxo_id": nft_utxo,
//             "charms": {
//                 "$00": {
//                     "name": "🗡️ Habit Tracker",
//                     "description": format!("Tracking habit: {}", habit_name),
//                     "owner": addr_str,
//                     "habit_name": habit_name.clone(),
//                     "total_sessions": current_sessions,
//                     "badges": get_badges_for_sessions(current_sessions),
//                 }
//             }
//         }],
//         "outs": [{
//             "address": addr_str,
//             "charms": {
//                 "$00": {
//                     "name": "🗡️ Habit Tracker",
//                     "description": format!("Tracking habit: {}", habit_name),
//                     "owner": addr_str,
//                     "habit_name": habit_name,
//                     "total_sessions": current_sessions + 1,
//                     "last_updated": chrono::Utc::now().timestamp(),
//                     "badges": get_badges_for_sessions(current_sessions + 1),
//                 }
//             },
//             "sats": NFT_AMOUNT_SATS
//         }]
//     });

//     log::debug!("\n Calling prover...");

//     // Auto-detect which prover backend to use
//     let txs = match backend {
//         ProverBackend::CliMock => {
//             // Use CLI mock for regtest
//             let contract_path = get_contract_path();
//             let prev_txs = vec![prev_tx_raw];

//             prove_with_cli(
//                 &spell,
//                 contract_path.to_str().unwrap(),
//                 &prev_txs,
//                 &funding_utxo,
//                 funding_value,
//                 &addr_str,
//                 DEFAULT_FEE_RATE,
//             )?
//         }
//         ProverBackend::_Http => {
//             // Use HTTP API for testnet/mainnet
//             let prev_txs = vec![json!({
//                 "bitcoin": prev_tx_raw
//             })];

//             let prover_request = json!({
//                 "version": 8,
//                 "spell": spell,
//                 "binaries": {vk: binary_base64},
//                 "prev_txs": prev_txs,
//                 "funding_utxo": funding_utxo,
//                 "funding_utxo_value": funding_value,
//                 "change_address": addr_str,
//                 "fee_rate": 2.0,
//                 "chain": "bitcoin"
//             });

//             let client = reqwest::Client::new();
//             let response = client
//                 .post("http://localhost:17784/spells/prove")
//                 .json(&prover_request)
//                 .timeout(std::time::Duration::from_secs(300))
//                 .send()
//                 .await?;

//             if !response.status().is_success() {
//                 let error = response.text().await?;
//                 anyhow::bail!("Prover error: {}", error);
//             }

//             response.json().await?
//         }
//     };

//     let bitcoin_txs: Vec<bitcoin::Transaction> = txs
//         .iter()
//         .filter_map(|tx| match tx {
//             Tx::Bitcoin(btx) => Some(btx.inner().clone()),
//             _ => None,
//         })
//         .collect();

//     let result = sign_and_broadcast_update(btc, bitcoin_txs, prev_txid, &nft_utxo)?;

//     if let Some(spell_txid) = result
//         .get("tx-results")
//         .and_then(|v| v.as_array())
//         .and_then(|arr| arr.get(1))
//         .and_then(|r| r.get("txid"))
//         .and_then(|v| v.as_str())
//     {
//         println!("\n NFT Updated!");
//         println!("   New UTXO: {}:0", spell_txid);
//         println!(
//             "   Sessions: {} → {}",
//             current_sessions,
//             current_sessions + 1
//         );
//         println!("\n To increment again:");
//         println!("   cargo run -- update --utxo {}:0", spell_txid);
//     }

//     Ok(())
// }

pub async fn update_nft(btc: &Client, nft_utxo: String) -> anyhow::Result<()> {
    println!("DEBUG: update_nft starting for UTXO: {}", &nft_utxo[..20]);
    log::info!("Updating NFT: {}", &nft_utxo[..12]);

    println!("DEBUG: Getting funding UTXO...");
    let (funding_utxo, funding_value, addr_str) = get_funding_utxo(btc, Some(&nft_utxo))?;
    println!("DEBUG: Got funding UTXO: {}", &funding_utxo[..20]);

    let (prev_txid, _) = nft_utxo
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("Invalid UTXO format"))?;

    println!("DEBUG: Extracting NFT metadata...");
    let (habit_name, current_sessions, _) = extract_nft_metadata(btc, prev_txid)?;
    println!("DEBUG: Current sessions: {}", current_sessions);

    println!("DEBUG: Getting previous transaction...");
    let prev_tx_raw = btc.get_raw_transaction_hex(&bitcoin::Txid::from_str(prev_txid)?, None)?;
    println!("DEBUG: Got prev tx");

    let (vk, _) = load_contract()?;
    let app_id = generate_app_id(&vk);

    println!("DEBUG: Creating update spell...");
    let spell = json!({
        "version": 8,
        "apps": {"$00": app_id},
        "ins": [{
            "utxo_id": nft_utxo.clone(),
            "charms": {
                "$00": {
                    "name": "🗡️ Habit Tracker",
                    "description": format!("Tracking habit: {}", habit_name),
                    "owner": addr_str,
                    "habit_name": habit_name.clone(),
                    "total_sessions": current_sessions,
                    "badges": get_badges_for_sessions(current_sessions),
                }
            }
        }],
        "outs": [{
            "address": addr_str,
            "charms": {
                "$00": {
                    "name": "🗡️ Habit Tracker",
                    "description": format!("Tracking habit: {}", habit_name),
                    "owner": addr_str,
                    "habit_name": habit_name,
                    "total_sessions": current_sessions + 1,
                    "last_updated": chrono::Utc::now().timestamp(),
                    "badges": get_badges_for_sessions(current_sessions + 1),
                }
            },
            "sats": NFT_AMOUNT_SATS
        }]
    });

    println!("DEBUG: Calling prover...");
    let contract_path = get_contract_path();
    let txs = prove_with_cli(
        &spell,
        contract_path.to_str().unwrap(),
        &[prev_tx_raw],
        &funding_utxo,
        funding_value,
        &addr_str,
        DEFAULT_FEE_RATE,
    )?;
    println!("DEBUG: Prover returned {} txs", txs.len());

    println!("DEBUG: Converting to bitcoin transactions...");
    let bitcoin_txs: Vec<bitcoin::Transaction> = txs
        .iter()
        .filter_map(|tx| match tx {
            Tx::Bitcoin(btx) => Some(btx.inner().clone()),
            _ => None,
        })
        .collect();
    println!("DEBUG: Converted to {} bitcoin txs", bitcoin_txs.len());

    println!("DEBUG: Signing and broadcasting...");
    let result = sign_and_broadcast_update(btc, bitcoin_txs, prev_txid, &nft_utxo)?;
    println!("DEBUG: Broadcast complete");

    if let Some(spell_txid) = result
        .get("tx-results")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|r| r.get("txid"))
        .and_then(|v| v.as_str())
    {
        let new_sessions = current_sessions + 1;
        let stage = if new_sessions < 23 {
            "DESTRUCTION"
        } else if new_sessions < 45 {
            "INSTALLATION"
        } else if new_sessions < 67 {
            "INTEGRATION"
        } else {
            "LEGENDARY"
        };

        println!("\n⚔️  SESSION COMPLETE");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("   Habit: {}", habit_name);
        println!("   Sessions: {} → {}/66", current_sessions, new_sessions);
        println!("   Stage: {}", stage);
        println!("   New UTXO: {}:0", spell_txid);

        // Check if new badge earned
        let new_badge = BADGE_MILESTONES
            .iter()
            .find(|(threshold, _)| *threshold == new_sessions)
            .map(|(_, badge)| *badge);

        if let Some(badge) = new_badge {
            println!("\n🏆 NEW BADGE UNLOCKED!");
            println!("   {}", badge);
        }

        println!("\nTo continue your journey:");
        println!("   cargo run -- update --utxo {}:0", spell_txid);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }

    Ok(())
}

pub fn update_nft_unsigned(
    btc: &Client,
    nft_utxo: String,
    user_address: String,
    funding_utxo: String,
    funding_value: u64,
) -> anyhow::Result<UnsignedUpdateResponse> {
    log::info!("Building unsigned NFT creation transactions");

    let (vk, _binary_base64) = load_contract()?;

    log::debug!(" User address: {}", user_address);
    log::debug!(" Funding UTXO: {} ({} sats)", funding_utxo, funding_value);
    log::debug!(" NFT UTXO: {}", nft_utxo);

    if funding_value < MIN_FUNDING_SATS {
        anyhow::bail!(
            "Insufficient funds. Have {} sats, need at least {} sats",
            funding_value,
            MIN_FUNDING_SATS
        );
    }

    // Extract current metadata
    let parts: Vec<&str> = nft_utxo.split(':').collect();
    let prev_txid = parts[0];

    let (habit_name, current_sessions, _) = extract_nft_metadata(btc, prev_txid)?;

    println!(" Current state: {} sessions", current_sessions);
    println!("  New state: {} sessions", current_sessions + 1);

    // Get previous transaction hex using the client
    let prev_tx_raw = btc.get_raw_transaction_hex(&bitcoin::Txid::from_str(prev_txid)?, None)?;
    let app_id = generate_app_id(&vk);

    let spell = json!({
        "version": 8,
        "apps": {"$00": app_id},
        "ins": [{
            "utxo_id": nft_utxo,
            "charms": {
                "$00": {
                    "name": "🗡️ Habit Tracker",
                    "description": format!("Tracking habit: {}", habit_name),
                    "owner": user_address,
                    "habit_name": habit_name.clone(),
                    "total_sessions": current_sessions,
                    "badges": get_badges_for_sessions(current_sessions),
                }
            }
        }],
        "outs": [{
            "address": user_address,
            "charms": {
                "$00": {
                    "name": "🗡️ Habit Tracker",
                    "description": format!("Tracking habit: {}", habit_name),
                    "owner": user_address,
                    "habit_name": habit_name,
                    "total_sessions": current_sessions + 1,
                    "last_updated": chrono::Utc::now().timestamp(),
                    "badges": get_badges_for_sessions(current_sessions + 1),
                }
            },
            "sats": NFT_AMOUNT_SATS
        }]
    });

    log::debug!("\n🔮 Calling prover...");

    let contract_path = get_contract_path();

    let prev_txs = vec![prev_tx_raw];

    let txs = prove_with_cli(
        &spell,
        contract_path.to_str().unwrap(),
        &prev_txs,
        &funding_utxo,
        funding_value,
        &user_address,
        DEFAULT_FEE_RATE,
    )?;

    log::debug!("   ✓ Got transactions from prover");

    let bitcoin_txs: Vec<bitcoin::Transaction> = txs
        .iter()
        .filter_map(|tx| match tx {
            Tx::Bitcoin(btx) => Some(btx.inner().clone()),
            _ => None,
        })
        .collect();

    let commit_tx = &bitcoin_txs[0];
    let spell_tx = &bitcoin_txs[1];

    // Extract signing info
    let signing_info = vec![
        // Commit tx - needs funding UTXO script
        SigningInputInfo {
            tx_index: 0,
            input_index: 0,
            prev_script_hex: "".to_string(),
            amount_sats: funding_value,
        },
        // Spell tx has 2 inputs: NFT UTXO + commit output
        // Input 0: NFT UTXO
        SigningInputInfo {
            tx_index: 1,
            input_index: 0,
            prev_script_hex: "".to_string(),
            amount_sats: 1000,
        },
        // Input 1: Commit output
        SigningInputInfo {
            tx_index: 1,
            input_index: 1,
            prev_script_hex: hex::encode(commit_tx.output[0].script_pubkey.as_bytes()),
            amount_sats: commit_tx.output[0].value.to_sat(),
        },
    ];

    Ok(UnsignedUpdateResponse {
        commit_tx_hex: hex::encode(bitcoin::consensus::serialize(commit_tx)),
        spell_tx_hex: hex::encode(bitcoin::consensus::serialize(spell_tx)),
        commit_txid: commit_tx.compute_txid().to_string(),
        spell_inputs_info: signing_info,
        current_sessions,
        new_sessions: current_sessions + 1,
    })
}

pub fn view_nft(btc: &Client, nft_utxo: String) -> anyhow::Result<()> {
    log::info!("Viewing NFT: {}", &nft_utxo[..12]);

    let (txid, vout) = nft_utxo
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("Invalid UTXO format"))?;

    let (habit_name, sessions, owner) = extract_nft_metadata(btc, txid)?;

    // Determine which stage the user is in
    let stage = if sessions < 23 {
        "🔴 Stage 1: DESTRUCTION"
    } else if sessions < 45 {
        "🟡 Stage 2: INSTALLATION"
    } else if sessions < 67 {
        "🟢 Stage 3: INTEGRATION"
    } else {
        "⭐ LEGENDARY TIER"
    };

    println!("\n⚔️  SAMURAI HABIT TRACKER");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   Habit: {}", habit_name);
    println!("   Sessions: {}/66", sessions);
    println!("   Stage: {}", stage);
    println!("   Owner: {}...", &owner[..20]);
    println!("   UTXO: {}:{}", txid, vout);

    // Progress bar
    let progress = if sessions <= 66 {
        (sessions as f64 / 66.0 * 30.0) as usize
    } else {
        30
    };
    let bar = "█".repeat(progress);
    let empty = "░".repeat(30 - progress);
    println!(
        "   Progress: [{}{}] {}%",
        bar,
        empty,
        (sessions as f64 / 66.0 * 100.0).min(100.0) as u8
    );

    // Show badges
    let badges = get_badges_for_sessions(sessions);
    if !badges.is_empty() {
        println!("\n🏆 BADGES EARNED:");
        for badge in &badges {
            println!("   {}", badge);
        }

        // Show next badge
        if let Some((next_sessions, next_badge)) = BADGE_MILESTONES
            .iter()
            .find(|(threshold, _)| *threshold > sessions)
        {
            println!("\n🎯 NEXT MILESTONE:");
            println!(
                "   {} sessions to unlock: {}",
                next_sessions - sessions,
                next_badge
            );
        } else if sessions >= 1000 {
            println!("\n🌟 You have achieved LIVING LEGEND status!");
            println!("   The path is yours. The blade is sharp. The Way is clear.");
        }
    } else {
        println!("\n   Begin your journey, warrior.");
        println!("   🌸 Complete your first session to earn 'First Blood'");
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}

// Function 1: Build unsigned transactions
pub fn create_nft_unsigned(
    habit_name: String,
    user_address: String,
    funding_utxo: String,
    funding_value: u64,
) -> anyhow::Result<UnsignedNftResponse> {
    log::debug!("🗡️  Building unsigned NFT transactions\n");

    // No need for btc client here - we're not signing or broadcasting
    let (vk, _binary_base64) = load_contract()?;

    log::debug!(" User address: {}", user_address);
    log::debug!(" Funding UTXO: {} ({} sats)", funding_utxo, funding_value);

    // Validate funds
    let min_required = 2000;
    if funding_value < min_required {
        anyhow::bail!(
            "Insufficient funds. Have {} sats, need at least {} sats",
            funding_value,
            min_required
        );
    }

    let identity_input = format!("habit_tracker_{}", chrono::Utc::now().timestamp());
    let mut hasher = Sha256::new();
    hasher.update(identity_input.as_bytes());
    let identity_hash = hasher.finalize();
    let identity_hex = hex::encode(identity_hash);
    let app_id = format!("n/{}/{}", identity_hex, vk);

    let spell = json!({
        "version": 8,
        "apps": {"$00": app_id},
        "ins": [],
        "outs": [{
            "address": user_address,
            "charms": {
                "$00": {
                    "name": "🗡️ Habit Tracker",
                    "description": format!("Tracking habit: {}", habit_name),
                    "owner": user_address,
                    "habit_name": habit_name,
                    "total_sessions": 0,
                    "created_at": chrono::Utc::now().timestamp(),
                    "badges": get_badges_for_sessions(0),
                }
            },
            "sats": NFT_AMOUNT_SATS
        }]
    });

    log::debug!("\n Calling prover...");

    let contract_path = get_contract_path();

    let txs = prove_with_cli(
        &spell,
        contract_path.to_str().unwrap(),
        &[],
        &funding_utxo,
        funding_value,
        &user_address,
        DEFAULT_FEE_RATE,
    )?;

    log::debug!("   ✓ Got transactions from prover");

    // Convert to bitcoin::Transaction objects
    let bitcoin_txs: Vec<bitcoin::Transaction> = txs
        .iter()
        .filter_map(|tx| match tx {
            Tx::Bitcoin(btx) => Some(btx.inner().clone()),
            _ => None,
        })
        .collect();

    let commit_tx = &bitcoin_txs[0];
    let spell_tx = &bitcoin_txs[1];

    // Extract signing info
    let signing_info = vec![
        // Commit tx - needs funding UTXO script
        SigningInputInfo {
            tx_index: 0,
            input_index: 0,
            prev_script_hex: "".to_string(),
            amount_sats: funding_value,
        },
        // Spell tx - needs commit output script
        SigningInputInfo {
            tx_index: 1,
            input_index: 0,
            prev_script_hex: hex::encode(commit_tx.output[0].script_pubkey.as_bytes()),
            amount_sats: commit_tx.output[0].value.to_sat(),
        },
    ];

    Ok(UnsignedNftResponse {
        commit_tx_hex: hex::encode(bitcoin::consensus::serialize(commit_tx)),
        spell_tx_hex: hex::encode(bitcoin::consensus::serialize(spell_tx)),
        commit_txid: commit_tx.compute_txid().to_string(),
        spell_inputs_info: signing_info,
    })
}

// Function 2: Broadcast signed transactions
pub fn broadcast_nft(
    btc: &Client,
    signed_commit_hex: String,
    signed_spell_hex: String,
) -> anyhow::Result<BroadcastNftResponse> {
    log::debug!("\n Broadcasting NFT transactions...");

    // Decode hex to bytes, then deserialize to Transaction
    let commit_bytes = hex::decode(&signed_commit_hex)?;
    let commit_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&commit_bytes)?;

    let spell_bytes = hex::decode(&signed_spell_hex)?;
    let spell_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&spell_bytes)?;

    // Broadcast commit first
    let commit_txid = btc.send_raw_transaction(&commit_tx)?;
    log::debug!("Commit tx: {}", commit_txid);

    // Broadcast spell
    let spell_txid = btc.send_raw_transaction(&spell_tx)?;
    log::debug!("Spell tx: {}", spell_txid);

    Ok(BroadcastNftResponse {
        commit_txid: commit_txid.to_string(),
        spell_txid: spell_txid.to_string(),
    })
}

// ============================================================================
// Transaction Signing & Broadcasting
// ============================================================================

pub fn sign_and_broadcast_create(
    btc: &Client,
    bitcoin_txs: Vec<bitcoin::Transaction>,
) -> anyhow::Result<serde_json::Value> {
    println!(
        "DEBUG: sign_and_broadcast_create: Starting with {} txs",
        bitcoin_txs.len()
    );
    log::debug!("Signing transactions");

    println!("DEBUG: Signing commit transaction...");
    let signed_commit = btc.sign_raw_transaction_with_wallet(&bitcoin_txs[0], None, None)?;
    if !signed_commit.complete {
        anyhow::bail!("Failed to sign commit transaction");
    }
    println!("DEBUG: Commit tx signed");

    let commit_tx = &bitcoin_txs[0];
    let commit_script_pubkey = commit_tx.output[0].script_pubkey.clone();
    let commit_amount_btc = commit_tx.output[0].value.to_btc();

    let prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: commit_tx.compute_txid(),
        vout: 0,
        script_pub_key: commit_script_pubkey,
        redeem_script: None,
        amount: Some(bitcoin::Amount::from_btc(commit_amount_btc)?),
    };

    println!("DEBUG: Signing spell transaction...");
    let signed_spell =
        btc.sign_raw_transaction_with_wallet(&bitcoin_txs[1], Some(&[prevout]), None)?;

    if !signed_spell.complete {
        anyhow::bail!("Failed to sign spell transaction");
    }
    println!("DEBUG: Spell tx signed");
    log::debug!("Broadcasting transactions");

    println!("DEBUG: Broadcasting commit tx...");
    let commit_txid = btc.send_raw_transaction(&signed_commit.hex)?;
    println!("DEBUG: Commit tx broadcast: {}", commit_txid);

    println!("DEBUG: Broadcasting spell tx...");
    let spell_txid = btc.send_raw_transaction(&signed_spell.hex)?;
    println!("DEBUG: Broadcasting commit tx...");

    log::info!("NFT created - Spell TXID: {}", spell_txid);

    let result = json!({
        "tx-results": [
            {"txid": commit_txid.to_string()},
            {"txid": spell_txid.to_string()},
        ]
    });

    Ok(result)
}

// pub fn sign_and_broadcast_update(
//     btc: &Client,
//     bitcoin_txs: Vec<bitcoin::Transaction>,
//     nft_txid: &str,
//     nft_utxo: &str,
// ) -> anyhow::Result<serde_json::Value> {
//     log::debug!("Signing update transactions");

//     // Sign commit transaction
//     let signed_commit = btc.sign_raw_transaction_with_wallet(&bitcoin_txs[0], None, None)?;
//     if !signed_commit.complete {
//         anyhow::bail!("Failed to sign commit transaction");
//     }

//     // Get NFT transaction details for signing
//     let nft_tx_raw = btc.get_raw_transaction(&bitcoin::Txid::from_str(nft_txid)?, None)?;
//     let nft_vout: u32 = nft_utxo.split(':').nth(1).unwrap().parse()?;

//     // Prepare prevouts for spell transaction (needs BOTH inputs)
//     let nft_prevout = bitcoincore_rpc::json::SignRawTransactionInput {
//         txid: bitcoin::Txid::from_str(nft_txid)?,
//         vout: nft_vout,
//         script_pub_key: nft_tx_raw.output[nft_vout as usize].script_pubkey.clone(),
//         redeem_script: None,
//         amount: Some(bitcoin::Amount::from_sat(1000)),
//     };

//     let commit_tx = &bitcoin_txs[0];
//     let commit_prevout = bitcoincore_rpc::json::SignRawTransactionInput {
//         txid: commit_tx.compute_txid(),
//         vout: 0,
//         script_pub_key: commit_tx.output[0].script_pubkey.clone(),
//         redeem_script: None,
//         amount: Some(commit_tx.output[0].value),
//     };

//     // Sign spell transaction with both prevouts
//     let signed_spell = btc.sign_raw_transaction_with_wallet(
//         &bitcoin_txs[1],
//         Some(&[nft_prevout, commit_prevout]),
//         None,
//     )?;

//     if !signed_spell.complete {
//         let errors = signed_spell.errors.unwrap_or_default();
//         for err in &errors {
//             eprintln!("   Signing error: {:?}", err);
//         }
//         anyhow::bail!("Failed to sign spell transaction. Errors: {:?}", errors);
//     }
//     println!("   ✓ Spell tx signed");

//     // Detect network and choose broadcast method
//     let network = btc.get_blockchain_info()?.chain;

//     match network {
//         bitcoincore_rpc::bitcoin::Network::Regtest => {
//             log::debug!("Broadcasting via submitpackage (regtest)");

//             let result = btc.call::<serde_json::Value>(
//                 "submitpackage",
//                 &[serde_json::json!([
//                     hex::encode(&signed_commit.hex),
//                     hex::encode(&signed_spell.hex),
//                 ])],
//             )?;

//             if let Some(results) = result.get("tx-results").and_then(|v| v.as_array()) {
//                 for (i, r) in results.iter().enumerate() {
//                     if let Some(txid) = r.get("txid") {
//                         let tx_type = if i == 0 { "Commit" } else { "Spell" };
//                         println!("   ✓ {} tx: {}", tx_type, txid.as_str().unwrap());
//                     }
//                     if let Some(err) = r.get("error") {
//                         anyhow::bail!("Package tx {} rejected: {}", i, err);
//                     }
//                 }
//             }

//             Ok(result)
//         }
//         _ => {
//             log::debug!("Broadcasting transactions sequentially");

//             let commit_txid = btc.send_raw_transaction(&signed_commit.hex)?;
//             let spell_txid = btc.send_raw_transaction(&signed_spell.hex)?;

//             log::info!("NFT updated - Spell TXID: {}", spell_txid);

//             Ok(json!({
//                 "tx-results": [
//                     {"txid": commit_txid.to_string()},
//                     {"txid": spell_txid.to_string()},
//                 ]
//             }))
//         }
//     }
// }

fn sign_and_broadcast_update(
    btc: &Client,
    bitcoin_txs: Vec<bitcoin::Transaction>,
    nft_txid: &str,
    nft_utxo: &str,
) -> anyhow::Result<serde_json::Value> {
    println!(
        "DEBUG: sign_and_broadcast_update: Starting with {} txs",
        bitcoin_txs.len()
    );
    log::debug!("Signing update transactions");

    println!("DEBUG: Signing commit transaction...");
    let signed_commit = btc.sign_raw_transaction_with_wallet(&bitcoin_txs[0], None, None)?;
    if !signed_commit.complete {
        anyhow::bail!("Failed to sign commit transaction");
    }
    println!("DEBUG: Commit tx signed");

    let nft_tx_raw = btc.get_raw_transaction(&bitcoin::Txid::from_str(nft_txid)?, None)?;
    let nft_vout: u32 = nft_utxo.split(':').nth(1).unwrap().parse()?;

    let nft_prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: bitcoin::Txid::from_str(nft_txid)?,
        vout: nft_vout,
        script_pub_key: nft_tx_raw.output[nft_vout as usize].script_pubkey.clone(),
        redeem_script: None,
        amount: Some(bitcoin::Amount::from_sat(NFT_AMOUNT_SATS)),
    };

    let commit_tx = &bitcoin_txs[0];
    let commit_prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: commit_tx.compute_txid(),
        vout: 0,
        script_pub_key: commit_tx.output[0].script_pubkey.clone(),
        redeem_script: None,
        amount: Some(commit_tx.output[0].value),
    };

    println!("DEBUG: Signing spell transaction...");
    let signed_spell = btc.sign_raw_transaction_with_wallet(
        &bitcoin_txs[1],
        Some(&[nft_prevout, commit_prevout]),
        None,
    )?;

    if !signed_spell.complete {
        let errors = signed_spell.errors.unwrap_or_default();
        anyhow::bail!("Failed to sign spell transaction: {:?}", errors);
    }
    println!("DEBUG: Spell tx signed");

    // Always use sequential broadcasting for updates (more reliable)
    println!("DEBUG: Broadcasting transactions sequentially...");

    println!("DEBUG: Broadcasting commit tx...");
    let commit_txid = btc.send_raw_transaction(&signed_commit.hex)?;
    println!("DEBUG: Commit tx broadcast: {}", commit_txid);

    println!("DEBUG: Broadcasting spell tx...");
    let spell_txid = btc.send_raw_transaction(&signed_spell.hex)?;
    println!("DEBUG: Spell tx broadcast: {}", spell_txid);

    log::info!("NFT updated - Spell TXID: {}", spell_txid);

    Ok(json!({
        "tx-results": [
            {"txid": commit_txid.to_string()},
            {"txid": spell_txid.to_string()},
        ]
    }))
}
