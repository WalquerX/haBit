// src/nft.rs
use base64::Engine;
use bitcoincore_rpc::bitcoin;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use charms_client::tx::Tx;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::process::Command;
use std::str::FromStr;
use tempfile::NamedTempFile;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProverBackend {
    Http,
    CliMock,
}

impl ProverBackend {
    pub fn auto_detect(btc: &Client) -> anyhow::Result<Self> {
        let info = btc.get_blockchain_info()?;
        match info.chain {
            bitcoincore_rpc::bitcoin::Network::Regtest => {
                println!("   🔧 Detected regtest - using CLI mock mode");
                Ok(ProverBackend::CliMock)
            }
            _ => {
                println!("   🌐 Detected {} - using HTTP API", info.chain);
                Ok(ProverBackend::Http)
            }
        }
    }
}

pub fn get_contract_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/habit-tracker.wasm")
}

pub fn get_contract_vk_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/habit-tracker.vk")
}

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
    // let vk = "328139bcb52b17730b8ed5f6658ff6329a51f2ba61f3cdccdb89b8443ab97486".to_string();

    Ok((vk, binary_base64))
}

pub fn connect_bitcoin() -> anyhow::Result<Client> {
    let cookie_path = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("No home dir"))?
        .join(".bitcoin/testnet4/.cookie");

    let btc = Client::new(
        "http://127.0.0.1:48332/wallet/test",
        Auth::CookieFile(cookie_path),
    )?;
    Ok(btc)
}

pub fn get_funding_utxo(
    btc: &Client,
    exclude_utxo: Option<&str>,
) -> anyhow::Result<(String, u64, String)> {
    let utxos = btc.list_unspent(None, None, None, None, None)?;
    let network = btc.get_blockchain_info()?.chain;

    let funding = utxos
        .iter()
        .filter(|utxo| {
            let utxo_id = format!("{}:{}", utxo.txid, utxo.vout);
            let is_nft = utxo.amount.to_sat() == 1000;
            let is_excluded = exclude_utxo.map_or(false, |excluded| utxo_id == excluded);
            !is_nft && !is_excluded
        })
        .next();

    if let Some(funding) = funding {
        let addr = funding
            .address
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Funding UTXO has no address"))?
            .clone()
            .require_network(network)?
            .to_string();

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

pub fn extract_nft_metadata(btc: &Client, txid: &str) -> anyhow::Result<(String, u64)> {
    println!("🔍 Extracting NFT metadata from {}...", txid);

    // Use the RPC client instead of bitcoin-cli
    let tx_hex = btc.get_raw_transaction_hex(&bitcoin::Txid::from_str(txid)?, None)?;

    let spell_output = Command::new("charms")
        .args(&["tx", "show-spell", "--tx", &tx_hex, "--mock", "--json"])
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

    println!("   📝 Habit: {}", habit_name);
    println!("   📊 Sessions: {}", sessions);

    Ok((habit_name, sessions))
}

pub fn sign_and_broadcast_create(
    btc: &Client,
    bitcoin_txs: Vec<bitcoin::Transaction>,
) -> anyhow::Result<serde_json::Value> {
    println!("\n📝 Signing transactions...");

    let signed_commit = btc.sign_raw_transaction_with_wallet(&bitcoin_txs[0], None, None)?;
    if !signed_commit.complete {
        anyhow::bail!("Failed to sign commit transaction");
    }
    println!("   ✓ Commit tx signed");

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

    let signed_spell =
        btc.sign_raw_transaction_with_wallet(&bitcoin_txs[1], Some(&[prevout]), None)?;

    if !signed_spell.complete {
        anyhow::bail!("Failed to sign spell transaction");
    }
    println!("   ✓ Spell tx signed");

    println!("\n📡 Broadcasting transactions...");

    let commit_txid = btc.send_raw_transaction(&signed_commit.hex)?;
    println!("   ✓ Commit tx broadcast: {}", commit_txid);

    let spell_txid = btc.send_raw_transaction(&signed_spell.hex)?;
    println!("   ✓ Spell tx broadcast: {}", spell_txid);

    let result = json!({
        "tx-results": [
            {"txid": commit_txid.to_string()},
            {"txid": spell_txid.to_string()},
        ]
    });

    Ok(result)
}

pub fn sign_and_broadcast(
    btc: &Client,
    bitcoin_txs: Vec<bitcoin::Transaction>,
) -> anyhow::Result<serde_json::Value> {
    println!("\n📝 Signing transactions...");

    let signed_commit = btc.sign_raw_transaction_with_wallet(&bitcoin_txs[0], None, None)?;
    if !signed_commit.complete {
        anyhow::bail!("Failed to sign commit transaction");
    }
    println!("   ✓ Commit tx signed");

    let commit_tx = &bitcoin_txs[0];
    let prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: commit_tx.compute_txid(),
        vout: 0,
        script_pub_key: commit_tx.output[0].script_pubkey.clone(),
        redeem_script: None,
        amount: Some(bitcoin::Amount::from_btc(
            commit_tx.output[0].value.to_btc(),
        )?),
    };

    let signed_spell =
        btc.sign_raw_transaction_with_wallet(&bitcoin_txs[1], Some(&[prevout]), None)?;
    if !signed_spell.complete {
        anyhow::bail!("Failed to sign spell transaction");
    }
    println!("   ✓ Spell tx signed");

    println!("\n📡 Broadcasting package...");

    let result = btc.call::<serde_json::Value>(
        "submitpackage",
        &[serde_json::json!([
            hex::encode(&signed_commit.hex),
            hex::encode(&signed_spell.hex),
        ])],
    )?;

    if let Some(results) = result.get("tx-results").and_then(|v| v.as_array()) {
        for (i, r) in results.iter().enumerate() {
            if let Some(err) = r.get("error") {
                anyhow::bail!("Package tx {} rejected: {}", i, err);
            }
        }
    }

    Ok(result)
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
    let mut spell_file = NamedTempFile::new()?;
    spell_file.write_all(serde_json::to_string_pretty(spell)?.as_bytes())?;
    let spell_path = spell_file.path().to_str().unwrap();

    let charms_bin = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("No home dir"))?
        .join("BOS/charms/target/release/charms");

    if !charms_bin.exists() {
        anyhow::bail!(
            "Local charms binary not found at {:?}\nBuild it: cd ~/BOS/charms && cargo build --release",
            charms_bin
        );
    }

    // Convert contract_path to absolute path
    let absolute_contract_path = std::fs::canonicalize(contract_path)?;
    println!("   📦 Using contract: {:?}", absolute_contract_path);

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

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("charms spell prove failed: {}", stderr);
    }

    let stdout = String::from_utf8(output.stdout)?;
    let txs: Vec<Tx> = serde_json::from_str(&stdout)
        .map_err(|e| anyhow::anyhow!("Failed to parse CLI output: {}", e))?;

    Ok(txs)
}

pub fn create_nft(btc: &Client, habit_name: String) -> anyhow::Result<()> {
    println!("🗡️  Creating Habit Tracker NFT\n");

    // let backend = ProverBackend::auto_detect(btc)?;
    let (vk, _binary_base64) = load_contract()?;
    // let network = btc.get_blockchain_info()?.chain;
    let utxos = btc.list_unspent(None, None, None, None, None)?;
    let funding = utxos.first().expect("No UTXOs!");

    let addr_str = funding
        .address
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Funding UTXO has no address"))?
        .clone()
        .assume_checked()
        .to_string();

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
            "sats": 1000
        }]
    });

    println!("\n🔮 Calling prover...");

    let contract_path = get_contract_path();

    let txs = prove_with_cli(
        &spell,
        contract_path.to_str().unwrap(),
        &[],
        &format!("{}:{}", funding.txid, funding.vout),
        funding.amount.to_sat(),
        &addr_str,
        2.0,
    )?;

    println!("   ✓ Got transactions from prover");

    let bitcoin_txs: Vec<bitcoin::Transaction> = txs
        .iter()
        .filter_map(|tx| match tx {
            Tx::Bitcoin(btx) => Some(btx.inner().clone()),
            _ => None,
        })
        .collect();

    println!(
        "   Commit tx: {} bytes",
        bitcoin::consensus::serialize(&bitcoin_txs[0]).len()
    );
    println!(
        "   Spell tx: {} bytes",
        bitcoin::consensus::serialize(&bitcoin_txs[1]).len()
    );

    let result = sign_and_broadcast_create(btc, bitcoin_txs)?;

    if let Some(spell_txid) = result
        .get("tx-results")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|r| r.get("txid"))
        .and_then(|v| v.as_str())
    {
        println!("\n✅ NFT Created!");
        println!("   UTXO: {}:0", spell_txid);
        println!("   Sessions: 0");
        println!("\nTo increment:");
        println!("   cargo run -- update --utxo {}:0", spell_txid);
    }

    Ok(())
}

pub async fn update_nft(btc: &Client, nft_utxo: String) -> anyhow::Result<()> {
    println!("🔄 Updating Habit Tracker NFT\n");

    // let backend = ProverBackend::auto_detect(btc)?;
    let backend = ProverBackend::CliMock;
    let (vk, binary_base64) = load_contract()?;
    let (funding_utxo, funding_value, addr_str) = get_funding_utxo(btc, Some(&nft_utxo))?;

    let parts: Vec<&str> = nft_utxo.split(':').collect();
    let prev_txid = parts[0];

    let (habit_name, current_sessions) = extract_nft_metadata(btc, prev_txid)?;

    println!("\n🔍 Fetching previous transaction...");

    let prev_tx_raw = btc.get_raw_transaction_hex(&bitcoin::Txid::from_str(prev_txid)?, None)?;

    let identity_input = format!("habit_tracker_{}", chrono::Utc::now().timestamp());
    let mut hasher = Sha256::new();
    hasher.update(identity_input.as_bytes());
    let identity_hash = hasher.finalize();
    let identity_hex = hex::encode(identity_hash);
    let app_id = format!("n/{}/{}", identity_hex, vk);

    let spell = json!({
        "version": 8,
        "apps": {"$00": app_id},
        "ins": [{
            "utxo_id": nft_utxo,
            "charms": {
                "$00": {
                    "name": "🗡️ Habit Tracker",
                    "description": format!("Tracking habit: {}", habit_name),
                    "owner": addr_str,
                    "habit_name": habit_name.clone(),
                    "total_sessions": current_sessions,
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
                }
            },
            "sats": 1000
        }]
    });

    println!("\n🔮 Calling prover...");

    // Auto-detect which prover backend to use
    let txs = match backend {
        ProverBackend::CliMock => {
            // Use CLI mock for regtest
            let contract_path = get_contract_path();
            let prev_txs = vec![prev_tx_raw];

            prove_with_cli(
                &spell,
                contract_path.to_str().unwrap(),
                &prev_txs,
                &funding_utxo,
                funding_value,
                &addr_str,
                2.0,
            )?
        }
        ProverBackend::Http => {
            // Use HTTP API for testnet/mainnet
            let prev_txs = vec![json!({
                "bitcoin": prev_tx_raw
            })];

            let prover_request = json!({
                "version": 8,
                "spell": spell,
                "binaries": {vk: binary_base64},
                "prev_txs": prev_txs,
                "funding_utxo": funding_utxo,
                "funding_utxo_value": funding_value,
                "change_address": addr_str,
                "fee_rate": 2.0,
                "chain": "bitcoin"
            });

            let client = reqwest::Client::new();
            let response = client
                .post("http://localhost:17784/spells/prove")
                .json(&prover_request)
                .timeout(std::time::Duration::from_secs(300))
                .send()
                .await?;

            if !response.status().is_success() {
                let error = response.text().await?;
                anyhow::bail!("Prover error: {}", error);
            }

            response.json().await?
        }
    };

    let bitcoin_txs: Vec<bitcoin::Transaction> = txs
        .iter()
        .filter_map(|tx| match tx {
            Tx::Bitcoin(btx) => Some(btx.inner().clone()),
            _ => None,
        })
        .collect();

    let result = sign_and_broadcast_update(btc, bitcoin_txs, prev_txid, &nft_utxo)?;
    // let result = match backend {
    //     ProverBackend::CliMock => {
    //         // Use sign_and_broadcast_create for regtest (broadcasts separately)
    //         sign_and_broadcast_create(btc, bitcoin_txs)?
    //     }
    //     ProverBackend::Http => {
    //         // Use sign_and_broadcast for production (uses submitpackage)
    //         sign_and_broadcast(btc, bitcoin_txs)?
    //     }
    // };

    if let Some(spell_txid) = result
        .get("tx-results")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.get(1))
        .and_then(|r| r.get("txid"))
        .and_then(|v| v.as_str())
    {
        println!("\n✅ NFT Updated!");
        println!("   New UTXO: {}:0", spell_txid);
        println!(
            "   Sessions: {} → {}",
            current_sessions,
            current_sessions + 1
        );
        println!("\nTo increment again:");
        println!("   cargo run -- update --utxo {}:0", spell_txid);
    }

    Ok(())
}

pub fn sign_and_broadcast_update(
    btc: &Client,
    bitcoin_txs: Vec<bitcoin::Transaction>,
    nft_txid: &str,
    nft_utxo: &str,
) -> anyhow::Result<serde_json::Value> {
    println!("\n📝 Signing transactions...");

    // Sign commit transaction
    let signed_commit = btc.sign_raw_transaction_with_wallet(&bitcoin_txs[0], None, None)?;
    if !signed_commit.complete {
        anyhow::bail!("Failed to sign commit transaction");
    }
    println!("   ✓ Commit tx signed");

    // Get NFT transaction details for signing
    let nft_tx_raw = btc.get_raw_transaction(&bitcoin::Txid::from_str(nft_txid)?, None)?;
    let nft_vout: u32 = nft_utxo.split(':').nth(1).unwrap().parse()?;

    // Prepare prevouts for spell transaction (needs BOTH inputs)
    let nft_prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: bitcoin::Txid::from_str(nft_txid)?,
        vout: nft_vout,
        script_pub_key: nft_tx_raw.output[nft_vout as usize].script_pubkey.clone(),
        redeem_script: None,
        amount: Some(bitcoin::Amount::from_sat(1000)),
    };

    let commit_tx = &bitcoin_txs[0];
    let commit_prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: commit_tx.compute_txid(),
        vout: 0,
        script_pub_key: commit_tx.output[0].script_pubkey.clone(),
        redeem_script: None,
        amount: Some(commit_tx.output[0].value),
    };

    // Sign spell transaction with both prevouts
    let signed_spell = btc.sign_raw_transaction_with_wallet(
        &bitcoin_txs[1],
        Some(&[nft_prevout, commit_prevout]),
        None,
    )?;

    if !signed_spell.complete {
        let errors = signed_spell.errors.unwrap_or_default();
        for err in &errors {
            eprintln!("   Signing error: {:?}", err);
        }
        anyhow::bail!("Failed to sign spell transaction. Errors: {:?}", errors);
    }
    println!("   ✓ Spell tx signed");

    // Detect network and choose broadcast method
    let network = btc.get_blockchain_info()?.chain;
    
    match network {
        bitcoincore_rpc::bitcoin::Network::Regtest => {
            // Regtest: use submitpackage
            println!("\n📡 Broadcasting package (regtest)...");
            
            let result = btc.call::<serde_json::Value>(
                "submitpackage",
                &[serde_json::json!([
                    hex::encode(&signed_commit.hex),
                    hex::encode(&signed_spell.hex),
                ])],
            )?;

            if let Some(results) = result.get("tx-results").and_then(|v| v.as_array()) {
                for (i, r) in results.iter().enumerate() {
                    if let Some(txid) = r.get("txid") {
                        let tx_type = if i == 0 { "Commit" } else { "Spell" };
                        println!("   ✓ {} tx: {}", tx_type, txid.as_str().unwrap());
                    }
                    if let Some(err) = r.get("error") {
                        anyhow::bail!("Package tx {} rejected: {}", i, err);
                    }
                }
            }

            Ok(result)
        }
        _ => {
            // Testnet/Mainnet: broadcast sequentially
            println!("\n📡 Broadcasting transactions sequentially...");
            
            let commit_txid = btc.send_raw_transaction(&signed_commit.hex)?;
            println!("   ✓ Commit tx: {}", commit_txid);

            let spell_txid = btc.send_raw_transaction(&signed_spell.hex)?;
            println!("   ✓ Spell tx: {}", spell_txid);

            Ok(json!({
                "tx-results": [
                    {"txid": commit_txid.to_string()},
                    {"txid": spell_txid.to_string()},
                ]
            }))
        }
    }
}

pub fn update_nft_unsigned(
    btc: &Client,
    nft_utxo: String,
    user_address: String,
    funding_utxo: String,
    funding_value: u64,
) -> anyhow::Result<UnsignedUpdateResponse> {
    println!("🔄 Building unsigned NFT update transactions\n");

    let (vk, _binary_base64) = load_contract()?;

    println!("📍 User address: {}", user_address);
    println!("💰 Funding UTXO: {} ({} sats)", funding_utxo, funding_value);
    println!("🎯 NFT UTXO: {}", nft_utxo);

    // Validate funds
    let min_required = 2000;
    if funding_value < min_required {
        anyhow::bail!(
            "Insufficient funds. Have {} sats, need at least {} sats",
            funding_value,
            min_required
        );
    }

    // Extract current metadata
    let parts: Vec<&str> = nft_utxo.split(':').collect();
    let prev_txid = parts[0];

    let (habit_name, current_sessions) = extract_nft_metadata(btc, prev_txid)?;

    println!("📊 Current state: {} sessions", current_sessions);
    println!("➡️  New state: {} sessions", current_sessions + 1);

    // Get previous transaction hex using the client
    let prev_tx_raw = btc.get_raw_transaction_hex(&bitcoin::Txid::from_str(prev_txid)?, None)?;

    let identity_input = format!("habit_tracker_{}", chrono::Utc::now().timestamp());
    let mut hasher = Sha256::new();
    hasher.update(identity_input.as_bytes());
    let identity_hash = hasher.finalize();
    let identity_hex = hex::encode(identity_hash);
    let app_id = format!("n/{}/{}", identity_hex, vk);

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
                }
            },
            "sats": 1000
        }]
    });

    println!("\n🔮 Calling prover...");

    let contract_path = get_contract_path();

    let prev_txs = vec![prev_tx_raw];

    let txs = prove_with_cli(
        &spell,
        contract_path.to_str().unwrap(),
        &prev_txs,
        &funding_utxo,
        funding_value,
        &user_address,
        2.0,
    )?;

    println!("   ✓ Got transactions from prover");

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
    let mut signing_info = vec![];

    // Commit tx - needs funding UTXO script
    signing_info.push(SigningInputInfo {
        tx_index: 0,
        input_index: 0,
        prev_script_hex: "".to_string(),
        amount_sats: funding_value,
    });

    // Spell tx has 2 inputs: NFT UTXO + commit output
    // Input 0: NFT UTXO
    signing_info.push(SigningInputInfo {
        tx_index: 1,
        input_index: 0,
        prev_script_hex: "".to_string(),
        amount_sats: 1000,
    });

    // Input 1: Commit output
    signing_info.push(SigningInputInfo {
        tx_index: 1,
        input_index: 1,
        prev_script_hex: hex::encode(commit_tx.output[0].script_pubkey.as_bytes()),
        amount_sats: commit_tx.output[0].value.to_sat(),
    });

    Ok(UnsignedUpdateResponse {
        commit_tx_hex: hex::encode(bitcoin::consensus::serialize(commit_tx)),
        spell_tx_hex: hex::encode(bitcoin::consensus::serialize(spell_tx)),
        commit_txid: commit_tx.compute_txid().to_string(),
        spell_inputs_info: signing_info,
        current_sessions,
        new_sessions: current_sessions + 1,
    })
}

// Helper to get raw tx without Client
// fn get_raw_transaction_hex_direct(txid: &str) -> anyhow::Result<String> {
//     let output = Command::new("bitcoin-cli")
//         .args(&["-noconf", "-regtest", "getrawtransaction", txid])
//         .output()?;

//     if !output.status.success() {
//         let stderr = String::from_utf8_lossy(&output.stderr);
//         anyhow::bail!("Failed to get raw transaction: {}", stderr);
//     }

//     Ok(String::from_utf8(output.stdout)?.trim().to_string())
// }

pub fn view_nft(btc: &Client, nft_utxo: String) -> anyhow::Result<()> {
    println!("👀 Viewing NFT: {}\n", nft_utxo);

    let parts: Vec<&str> = nft_utxo.split(':').collect();
    let txid = parts[0];
    let vout = parts[1];

    let (habit_name, sessions) = extract_nft_metadata(btc, txid)?;

    println!("\n📊 NFT Details:");
    println!("   UTXO: {}", nft_utxo);
    println!("   Habit: {}", habit_name);
    println!("   Total Sessions: {}", sessions);
    println!("   Output: {}", vout);

    Ok(())
}

use serde::Serialize;

#[derive(Serialize)]
pub struct UnsignedNftResponse {
    pub commit_tx_hex: String,
    pub spell_tx_hex: String,
    pub commit_txid: String, // For reference
    pub spell_inputs_info: Vec<SigningInputInfo>,
}

#[derive(Serialize)]
pub struct UnsignedUpdateResponse {
    pub commit_tx_hex: String,
    pub spell_tx_hex: String,
    pub commit_txid: String,
    pub spell_inputs_info: Vec<SigningInputInfo>,
    pub current_sessions: u64,
    pub new_sessions: u64,
}

#[derive(Serialize)]
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

// Function 1: Build unsigned transactions
pub fn create_nft_unsigned(
    habit_name: String,
    user_address: String,
    funding_utxo: String,
    funding_value: u64,
) -> anyhow::Result<UnsignedNftResponse> {
    println!("🗡️  Building unsigned NFT transactions\n");

    // No need for btc client here - we're not signing or broadcasting
    let (vk, _binary_base64) = load_contract()?;

    println!("📍 User address: {}", user_address);
    println!("💰 Funding UTXO: {} ({} sats)", funding_utxo, funding_value);

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
                }
            },
            "sats": 1000
        }]
    });

    println!("\n🔮 Calling prover...");

    let contract_path = get_contract_path();

    let txs = prove_with_cli(
        &spell,
        contract_path.to_str().unwrap(),
        &[],
        &funding_utxo,
        funding_value,
        &user_address,
        2.0,
    )?;

    println!("   ✓ Got transactions from prover");

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
    let mut signing_info = vec![];

    // Commit tx - needs funding UTXO script
    // We need to fetch this or have frontend provide it
    signing_info.push(SigningInputInfo {
        tx_index: 0,
        input_index: 0,
        prev_script_hex: "".to_string(), // Frontend knows this from their UTXO
        amount_sats: funding_value,
    });

    // Spell tx - needs commit output script
    signing_info.push(SigningInputInfo {
        tx_index: 1,
        input_index: 0,
        prev_script_hex: hex::encode(commit_tx.output[0].script_pubkey.as_bytes()),
        amount_sats: commit_tx.output[0].value.to_sat(),
    });

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
    println!("\n📡 Broadcasting NFT transactions...");

    // Decode hex to bytes, then deserialize to Transaction
    let commit_bytes = hex::decode(&signed_commit_hex)?;
    let commit_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&commit_bytes)?;

    let spell_bytes = hex::decode(&signed_spell_hex)?;
    let spell_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&spell_bytes)?;

    // Broadcast commit first
    let commit_txid = btc.send_raw_transaction(&commit_tx)?;
    println!("   ✓ Commit tx: {}", commit_txid);

    // Broadcast spell
    let spell_txid = btc.send_raw_transaction(&spell_tx)?;
    println!("   ✓ Spell tx: {}", spell_txid);

    Ok(BroadcastNftResponse {
        commit_txid: commit_txid.to_string(),
        spell_txid: spell_txid.to_string(),
    })
}
