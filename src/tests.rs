use crate::*;
use bitcoincore_rpc::bitcoin;
use bitcoincore_rpc::bitcoin::Txid;
use bitcoincore_rpc::RpcApi;
use corepc_node::{Conf, Node};
use serde_json::json;
use serde_json::Value;
use serial_test::serial;
use std::{env, sync::Once, time::Instant};

// use super::*;
// use std::sync::Once;
// use corepc_node::{Conf, Node};

static INIT: Once = Once::new();
static mut NODE: Option<Node> = None;

/// Initialize Bitcoin node once for all tests
fn get_or_init_bitcoin_node() -> &'static Node {
    unsafe {
        INIT.call_once(|| {
            // Set environment variables
            std::env::set_var("TEMPDIR_ROOT", "/dev/shm");

            let mut conf = Conf::default();
            conf.args = vec!["-regtest", "-fallbackfee=0.0001", "-txindex=1"];
            conf.tmpdir = None;

            println!("Starting Bitcoin regtest node for tests...");
            let node =
                Node::from_downloaded_with_conf(&conf).expect("Failed to start Bitcoin node");

            // Mine initial blocks to get spendable coins
            let mining_addr = node
                .client
                .get_new_address(None, None)
                .expect("get mining address")
                .into_model()
                .expect("convert address")
                .0
                .assume_checked();

            node.client
                .generate_to_address(101, &mining_addr)
                .expect("generate initial blocks");

            println!("Bitcoin node ready at {}", node.rpc_url());
            NODE = Some(node);
        });

        NODE.as_ref().unwrap()
    }
}

//static INIT: Once = Once::new();

fn setup_bitcoin_env_vars() {
    INIT.call_once(|| {
        std::env::set_var("BITCOIN_RPC_URL", "http://127.0.0.1:18443");
        std::env::set_var("BITCOIN_RPC_USER", "user");
        std::env::set_var("BITCOIN_RPC_PASSWORD", "password");
        std::env::set_var("BITCOIN_NETWORK", "regtest");
        std::env::set_var("BITCOIN_FEE_SATS", "1000");
        std::env::set_var("BITCOIN_RPC_TIMEOUT_SECS", "10");
    });
}

fn start_bitcoin_node() -> Node {
    if env::var("TEMPDIR_ROOT").is_err() {
        env::set_var("TEMPDIR_ROOT", "/dev/shm");
    }

    let mut conf = Conf::default();
    conf.args = vec!["-regtest", "-fallbackfee=0.0001", "-txindex=1"];
    conf.tmpdir = None;

    println!("Instantiating Bitcoin node...");
    let node_start = Instant::now();
    let node =
        Node::from_downloaded_with_conf(&conf).expect("Failed to download and start Bitcoin node");
    println!("Bitcoin node instantiated in {:?}", node_start.elapsed());

    println!("Bitcoin RPC URL set to: {}", node.rpc_url());

    println!("Setting up mining environment...");
    let t = Instant::now();
    let mining_addr = node
        .client
        .get_new_address(None, None)
        .expect("get mining address");
    let mining_address = mining_addr
        .into_model()
        .expect("convert address")
        .0
        .assume_checked();

    // Generate enough blocks to have spendable coinbase outputs
    let _blocks = node
        .client
        .generate_to_address(101, &mining_address)
        .expect("generate initial blocks");
    println!("Mining setup completed in {:?}", t.elapsed());

    node
}

/// Helper to get a funded address for testing
fn get_funded_address(node: &Node) -> String {
    let addr = node
        .client
        .get_new_address(None, None)
        .expect("get new address")
        .into_model()
        .expect("convert address")
        .0
        .assume_checked();

    // Send some funds
    let amount = bitcoin::Amount::from_btc(1.0).expect("valid amount");
    node.client
        .send_to_address(&addr, amount)
        .expect("send to address");

    // Mine block to confirm
    let mining_addr = node
        .client
        .get_new_address(None, None)
        .expect("get mining address")
        .into_model()
        .expect("convert address")
        .0
        .assume_checked();

    node.client
        .generate_to_address(1, &mining_addr)
        .expect("mine confirmation block");

    addr.to_string()
}

use bitcoincore_rpc::{Auth, Client as BitcoinCoreClient};

fn get_bitcoincore_rpc_client(node: &Node) -> anyhow::Result<BitcoinCoreClient> {
    let params = &node.params;

    let cookie_values = params
        .get_cookie_values()?
        .ok_or_else(|| anyhow::anyhow!("No cookie values"))?;

    let base_url = format!("http://{}", params.rpc_socket);
    let base_client = BitcoinCoreClient::new(
        &base_url,
        Auth::UserPass(cookie_values.user.clone(), cookie_values.password.clone()),
    )?;

    let wallet_name = "test";

    // Try simple create wallet first
    match base_client.create_wallet(wallet_name, None, None, None, None) {
        Ok(_) => {
            println!("✓ Created wallet");

            // Now set it to descriptor mode using upgradewallet
            let wallet_url = format!("http://{}/wallet/{}", params.rpc_socket, wallet_name);
            let wallet_client = BitcoinCoreClient::new(
                &wallet_url,
                Auth::UserPass(cookie_values.user.clone(), cookie_values.password.clone()),
            )?;

            // Try to upgrade to descriptor wallet
            let _ = wallet_client.call::<serde_json::Value>("upgradewallet", &[json!(169900)]);

            Ok(wallet_client)
        }
        Err(e) => {
            println!("⚠ Create failed ({}), trying to load existing wallet...", e);

            // Try to load existing wallet
            let _ = base_client.load_wallet(wallet_name);

            let wallet_url = format!("http://{}/wallet/{}", params.rpc_socket, wallet_name);
            let wallet_client = BitcoinCoreClient::new(
                &wallet_url,
                Auth::UserPass(cookie_values.user, cookie_values.password),
            )?;

            Ok(wallet_client)
        }
    }
}

fn print_spell(client: &bitcoincore_rpc::Client, txid: &Txid) -> anyhow::Result<()> {
    // Get raw transaction hex from RPC
    let tx_hex = client.get_raw_transaction_hex(txid, None)?;

    // Run charms CLI to decode the spell
    let output = std::process::Command::new("charms")
        .args(&["tx", "show-spell", "--tx", &tx_hex, "--mock", "--json"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to decode spell: {}", stderr);
    }

    // Parse JSON output
    let spell: Value = serde_json::from_slice(&output.stdout)?;

    // Pretty-print the spell
    println!(
        "Spell for tx {}:\n{}",
        txid,
        serde_json::to_string_pretty(&spell)?
    );

    Ok(())
}

#[test]
#[serial]
fn create_nft_works() {
    println!("\n Testing NFT Creation with Unsigned/Broadcast Flow\n");

    // Setup: Initialize regtest node and fund wallet
    let node = get_or_init_bitcoin_node();
    let btc_client = get_bitcoincore_rpc_client(node).expect("create bitcoincore_rpc client");

    let info = btc_client.get_blockchain_info().unwrap();
    println!("✓ Bitcoin Core version: {:?}", info);

    // Create and fund a user address
    let user_addr = btc_client
        .get_new_address(None, None)
        .expect("get new address")
        .require_network(bitcoin::Network::Regtest)
        .expect("check network");

    println!(" User address: {}", user_addr);

    println!("⛏️  Generating blocks for wallet funds...");
    node.client
        .generate_to_address(101, &user_addr)
        .expect("generate blocks to wallet");

    // Get UTXOs for user
    let utxos = btc_client
        .list_unspent(None, None, None, None, None)
        .expect("list unspent");
    assert!(!utxos.is_empty(), "Wallet should have UTXOs after mining");

    let funding_utxo = utxos.first().expect("at least one UTXO");
    let funding_utxo_id = format!("{}:{}", funding_utxo.txid, funding_utxo.vout);
    let funding_value = funding_utxo.amount.to_sat();

    println!("✓ Wallet funded with {} UTXOs", utxos.len());
    println!(" Using UTXO: {} ({} sats)", funding_utxo_id, funding_value);

    // Verify contract exists
    let contract_path = get_contract_path();

    if !contract_path.exists() {
        println!("⚠ Contract not found at {:?}", contract_path);
        println!("  Run: make contract");
        panic!("Contract WASM required for test");
    }
    println!("✓ Contract found at {:?}", contract_path);

    // ========================================
    // STEP 1: Create unsigned transactions
    // ========================================
    println!("\n STEP 1: Creating unsigned transactions...");

    let habit_name = "Morning Meditation".to_string();
    let unsigned_result = create_nft_unsigned(
        habit_name.clone(),
        user_addr.to_string(),
        funding_utxo_id.clone(),
        funding_value,
    );

    assert!(
        unsigned_result.is_ok(),
        "create_nft_unsigned should succeed"
    );

    let unsigned = unsigned_result.unwrap();
    println!("✓ Unsigned transactions created:");
    println!("   Commit tx: {} bytes", unsigned.commit_tx_hex.len() / 2);
    println!("   Spell tx: {} bytes", unsigned.spell_tx_hex.len() / 2);
    println!("   Commit txid: {}", unsigned.commit_txid);
    println!(
        "   Signing info: {} inputs",
        unsigned.spell_inputs_info.len()
    );

    // Verify the structure
    assert!(
        !unsigned.commit_tx_hex.is_empty(),
        "commit_tx_hex should not be empty"
    );
    assert!(
        !unsigned.spell_tx_hex.is_empty(),
        "spell_tx_hex should not be empty"
    );
    assert!(
        !unsigned.commit_txid.is_empty(),
        "commit_txid should not be empty"
    );
    assert_eq!(
        unsigned.spell_inputs_info.len(),
        2,
        "should have 2 signing inputs"
    );

    // ========================================
    // STEP 2: Sign transactions (simulate frontend wallet)
    // ========================================
    println!("\n  STEP 2: Signing transactions (simulating wallet)...");

    // Decode the unsigned transactions
    let commit_bytes = hex::decode(&unsigned.commit_tx_hex).expect("decode commit hex");
    let commit_tx: bitcoin::Transaction =
        bitcoin::consensus::deserialize(&commit_bytes).expect("deserialize commit tx");

    let spell_bytes = hex::decode(&unsigned.spell_tx_hex).expect("decode spell hex");
    let spell_tx: bitcoin::Transaction =
        bitcoin::consensus::deserialize(&spell_bytes).expect("deserialize spell tx");

    println!("✓ Decoded transactions");
    println!("   Commit inputs: {}", commit_tx.input.len());
    println!("   Spell inputs: {}", spell_tx.input.len());

    // Sign commit transaction using Bitcoin Core wallet
    let signed_commit = btc_client
        .sign_raw_transaction_with_wallet(&commit_tx, None, None)
        .expect("sign commit tx");

    assert!(signed_commit.complete, "Commit tx signing should complete");
    println!("✓ Commit tx signed");

    // Sign spell transaction (needs prevout info for commit output)
    let commit_script_pubkey = commit_tx.output[0].script_pubkey.clone();
    let commit_amount = commit_tx.output[0].value;

    let prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: commit_tx.compute_txid(),
        vout: 0,
        script_pub_key: commit_script_pubkey,
        redeem_script: None,
        amount: Some(commit_amount),
    };

    let signed_spell = btc_client
        .sign_raw_transaction_with_wallet(&spell_tx, Some(&[prevout]), None)
        .expect("sign spell tx");

    assert!(signed_spell.complete, "Spell tx signing should complete");
    println!("✓ Spell tx signed");

    // ========================================
    // STEP 3: Broadcast signed transactions
    // ========================================
    println!("\n📡 STEP 3: Broadcasting signed transactions...");

    let broadcast_result = broadcast_nft(
        &btc_client,
        hex::encode(&signed_commit.hex), // ← hex encode Vec<u8> to String
        hex::encode(&signed_spell.hex),  // ← hex encode Vec<u8> to String
    );

    assert!(
        broadcast_result.is_ok(),
        "broadcast_nft should succeed: {:?}",
        broadcast_result.err()
    );

    let broadcast_response = broadcast_result.unwrap();
    println!("✓ Transactions broadcasted:");
    println!("   Commit txid: {}", broadcast_response.commit_txid);
    println!("   Spell txid: {}", broadcast_response.spell_txid);

    // ========================================
    // STEP 4: Mine block to confirm
    // ========================================
    println!("\n  STEP 4: Mining confirmation block...");

    let dummy_addr = node
        .client
        .get_new_address(None, None)
        .expect("get dummy address")
        .into_model()
        .expect("convert address")
        .0
        .assume_checked();

    node.client
        .generate_to_address(1, &dummy_addr)
        .expect("generate confirmation block");

    println!("✓ Block mined");

    // ========================================
    // STEP 5: Verify NFT was created
    // ========================================
    println!("\n✅ STEP 5: Verifying NFT creation...");

    let new_utxos = btc_client
        .list_unspent(None, None, None, None, None)
        .expect("list unspent after creation");

    // Look for the NFT UTXO (1000 sats)
    let nft_utxo = new_utxos.iter().find(|utxo| utxo.amount.to_sat() == 1000);

    assert!(
        nft_utxo.is_some(),
        "Should have created NFT UTXO with 1000 sats"
    );

    let nft = nft_utxo.unwrap();
    let nft_id = format!("{}:{}", nft.txid, nft.vout);
    println!("✓ NFT created at UTXO: {}", nft_id);

    // Verify the spell in the transaction
    let tx_hex = btc_client
        .get_raw_transaction_hex(&nft.txid, None)
        .expect("get raw tx");

    let output = std::process::Command::new("charms")
        .args(&["tx", "show-spell", "--tx", &tx_hex, "--mock", "--json"])
        .output()
        .expect("run charms");

    assert!(
        output.status.success(),
        "Charms should decode tx successfully"
    );

    let spell: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse spell JSON");

    // Verify spell contains charms
    let has_charms = spell
        .get("outs")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|o| o.get("charms").is_some()))
        .unwrap_or(false);

    assert!(has_charms, "Transaction must contain charms in spell");

    // Print the full spell for inspection
    print_spell(&btc_client, &nft.txid).expect("print spell");

    println!("\n TEST PASSED: Complete unsigned/broadcast flow successful!");
}

#[serial]
fn update_nft_works() {
    println!("\n Testing NFT Update with Unsigned/Broadcast Flow\n");

    // Setup
    let node = get_or_init_bitcoin_node();
    let btc_client = get_bitcoincore_rpc_client(node).expect("create bitcoincore_rpc client");

    let info = btc_client.get_blockchain_info().unwrap();
    println!("✓ Bitcoin Core version: {:?}", info);

    // Fund wallet
    let user_addr = btc_client
        .get_new_address(None, None)
        .expect("get new address")
        .require_network(bitcoin::Network::Regtest)
        .expect("check network");

    println!(" User address: {}", user_addr);

    println!("⛏️  Generating blocks for wallet funds...");
    node.client
        .generate_to_address(101, &user_addr)
        .expect("generate blocks to wallet");

    // ========================================
    // PREREQUISITE: Create an NFT first
    // ========================================
    println!("\n PREREQUISITE: Creating initial NFT...");

    let habit_name = "Update NFT Works Test".to_string();
    create_nft(&btc_client, habit_name.clone()).expect("create NFT");

    // Mine block to confirm THE NFT TRANSACTION
    let mining_addr = node
        .client
        .get_new_address(None, None)
        .expect("get mining address")
        .into_model()
        .expect("convert address")
        .0
        .assume_checked();

    println!("⛏️  Mining block to confirm NFT...");
    node.client
        .generate_to_address(1, &mining_addr)
        .expect("generate confirmation block");

    // Wait a moment for the block to be processed
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Find the NFT UTXO
    let utxos = btc_client
        .list_unspent(None, None, None, None, None)
        .expect("list unspent");

    let nft_utxos: Vec<_> = utxos.iter().filter(|u| u.amount.to_sat() == 1000).collect();

    println!("   Found {} NFT UTXOs total", nft_utxos.len());

    let nft_utxo = nft_utxos
        .iter()
        .find(|utxo| {
            if let Ok((habit, sessions)) = extract_nft_metadata(&btc_client, &utxo.txid.to_string())
            {
                habit == habit_name && sessions == 0
            } else {
                false
            }
        })
        .expect("Should find NFT with correct habit name and 0 sessions");

    let nft_utxo_id = format!("{}:{}", nft_utxo.txid, nft_utxo.vout);
    println!("✓ NFT created at: {}", nft_utxo_id);

    // Verify the NFT transaction is confirmed and has spell data
    println!(" Verifying NFT is confirmed...");
    let tx_info = btc_client
        .get_transaction(&nft_utxo.txid, None)
        .expect("get transaction info");

    println!("   Confirmations: {}", tx_info.info.confirmations);
    assert!(tx_info.info.confirmations > 0, "NFT should be confirmed");

    // Get funding UTXO (non-NFT)
    let funding_utxo = utxos
        .iter()
        .find(|u| u.amount.to_sat() != 1000)
        .expect("No funding UTXO found");

    let funding_utxo_id = format!("{}:{}", funding_utxo.txid, funding_utxo.vout);
    let funding_value = funding_utxo.amount.to_sat();

    println!(
        " Using funding UTXO: {} ({} sats)",
        funding_utxo_id, funding_value
    );

    // ========================================
    // STEP 1: Create unsigned update transactions
    // ========================================
    println!("\n STEP 1: Creating unsigned update transactions...");

    let unsigned_result = update_nft_unsigned(
        &btc_client,
        nft_utxo_id.clone(),
        user_addr.to_string(),
        funding_utxo_id.clone(),
        funding_value,
    );

    // Show the actual error if it fails
    if let Err(ref e) = unsigned_result {
        println!("   Error creating unsigned update: {}", e);
        println!("   Error chain:");
        let mut current = e.source();
        while let Some(cause) = current {
            println!("   - Caused by: {}", cause);
            current = cause.source();
        }
    }

    assert!(
        unsigned_result.is_ok(),
        "update_nft_unsigned should succeed: {:?}",
        unsigned_result.err()
    );

    let unsigned = unsigned_result.unwrap();
    println!("   Unsigned transactions created:");
    println!("   Commit tx: {} bytes", unsigned.commit_tx_hex.len() / 2);
    println!("   Spell tx: {} bytes", unsigned.spell_tx_hex.len() / 2);
    println!("   Current sessions: {}", unsigned.current_sessions);
    println!("   New sessions: {}", unsigned.new_sessions);
    println!(
        "   Signing info: {} inputs",
        unsigned.spell_inputs_info.len()
    );

    assert_eq!(unsigned.current_sessions, 0, "Should start at 0 sessions");
    assert_eq!(unsigned.new_sessions, 1, "Should increment to 1 session");
    assert_eq!(
        unsigned.spell_inputs_info.len(),
        3,
        "Should have 3 signing inputs"
    );

    // ========================================
    // STEP 2: Sign transactions
    // ========================================
    println!("\n  STEP 2: Signing transactions (simulating wallet)...");

    // Decode unsigned transactions
    let commit_bytes = hex::decode(&unsigned.commit_tx_hex).expect("decode commit hex");
    let commit_tx: bitcoin::Transaction =
        bitcoin::consensus::deserialize(&commit_bytes).expect("deserialize commit tx");

    let spell_bytes = hex::decode(&unsigned.spell_tx_hex).expect("decode spell hex");
    let spell_tx: bitcoin::Transaction =
        bitcoin::consensus::deserialize(&spell_bytes).expect("deserialize spell tx");

    println!("✓ Decoded transactions");
    println!("   Commit inputs: {}", commit_tx.input.len());
    println!("   Spell inputs: {}", spell_tx.input.len());

    // Sign commit transaction
    let signed_commit = btc_client
        .sign_raw_transaction_with_wallet(&commit_tx, None, None)
        .expect("sign commit tx");

    assert!(signed_commit.complete, "Commit tx signing should complete");
    println!("✓ Commit tx signed");

    // Sign spell transaction (needs prevouts for NFT and commit outputs)
    let nft_tx_raw = btc_client
        .get_raw_transaction(&nft_utxo.txid, None)
        .expect("get NFT transaction");

    let nft_prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: nft_utxo.txid,
        vout: nft_utxo.vout,
        script_pub_key: nft_tx_raw.output[nft_utxo.vout as usize]
            .script_pubkey
            .clone(),
        redeem_script: None,
        amount: Some(bitcoin::Amount::from_sat(1000)),
    };

    let commit_prevout = bitcoincore_rpc::json::SignRawTransactionInput {
        txid: commit_tx.compute_txid(),
        vout: 0,
        script_pub_key: commit_tx.output[0].script_pubkey.clone(),
        redeem_script: None,
        amount: Some(commit_tx.output[0].value),
    };

    let signed_spell = btc_client
        .sign_raw_transaction_with_wallet(&spell_tx, Some(&[nft_prevout, commit_prevout]), None)
        .expect("sign spell tx");

    assert!(signed_spell.complete, "Spell tx signing should complete");
    println!("  Spell tx signed");

    // ========================================
    // STEP 3: Broadcast signed transactions
    // ========================================
    println!("\n STEP 3: Broadcasting signed transactions...");

    let broadcast_result = broadcast_nft(
        &btc_client,
        hex::encode(&signed_commit.hex),
        hex::encode(&signed_spell.hex),
    );

    assert!(
        broadcast_result.is_ok(),
        "broadcast_nft should succeed: {:?}",
        broadcast_result.err()
    );

    let broadcast_response = broadcast_result.unwrap();
    println!("✓ Transactions broadcasted:");
    println!("   Commit txid: {}", broadcast_response.commit_txid);
    println!("   Spell txid: {}", broadcast_response.spell_txid);

    // ========================================
    // STEP 4: Mine block to confirm
    // ========================================
    println!("\n  STEP 4: Mining confirmation block...");

    node.client
        .generate_to_address(1, &mining_addr)
        .expect("generate confirmation block");

    println!("✓ Block mined");

    // ========================================
    // STEP 5: Verify NFT was updated
    // ========================================
    println!("\n STEP 5: Verifying NFT update...");

    // Use the spell txid from the broadcast response
    let new_nft_id = format!("{}:0", broadcast_response.spell_txid);

    println!("✓ NFT updated to: {}", new_nft_id);

    // Verify metadata from the correct transaction
    let (_habit_name, sessions) = extract_nft_metadata(&btc_client, &broadcast_response.spell_txid)
        .expect("extract metadata");

    assert_ne!(
        nft_utxo_id, new_nft_id,
        "NFT UTXO should be different after update"
    );

    assert_eq!(sessions, 1, "Sessions should be incremented to 1");
    println!("✓ Sessions incremented: 0 → {}", sessions);

    println!("\n TEST PASSED: Complete unsigned update/broadcast flow successful!");
}

// ============================================================================
// CLI TESTS
// ============================================================================
use std::time::Duration;

#[test]
#[serial]
fn cli_create_nft_works() {
    println!("\n🧪 Testing CLI: create command\n");

    let node = get_or_init_bitcoin_node();
    let btc_client = get_bitcoincore_rpc_client(node).expect("create client");

    // Fund wallet
    let user_addr = btc_client
        .get_new_address(None, None)
        .expect("get new address")
        .require_network(bitcoin::Network::Regtest)
        .expect("check network");

    println!("⛏️  Generating blocks for wallet funds...");
    node.client
        .generate_to_address(101, &user_addr)
        .expect("generate blocks");

    // Verify contract exists
    let contract_path = get_contract_path();
    if !contract_path.exists() {
        panic!("Contract WASM required. Run: make contract");
    }

    println!("📝 Creating NFT via CLI...");
    let habit_name = "CLI Test Habit".to_string();

    let result = create_nft(&btc_client, habit_name.clone());

    assert!(
        result.is_ok(),
        "CLI create_nft should succeed: {:?}",
        result.err()
    );

    // Mine block to confirm
    let mining_addr = node
        .client
        .get_new_address(None, None)
        .expect("get mining address")
        .into_model()
        .expect("convert address")
        .0
        .assume_checked();

    node.client
        .generate_to_address(1, &mining_addr)
        .expect("mine block");

    std::thread::sleep(std::time::Duration::from_millis(500));

    // Find NFT by habit name (NEW APPROACH - for consistency)
    let utxos = btc_client
        .list_unspent(None, None, None, None, None)
        .expect("list unspent");

    let nft_utxos: Vec<_> = utxos.iter().filter(|u| u.amount.to_sat() == 1000).collect();

    let nft_utxo = nft_utxos
        .iter()
        .find(|utxo| {
            if let Ok((habit, _)) = extract_nft_metadata(&btc_client, &utxo.txid.to_string()) {
                habit == habit_name
            } else {
                false
            }
        })
        .expect("Should find NFT with correct habit name");

    let nft_id = format!("{}:{}", nft_utxo.txid, nft_utxo.vout);

    // Verify metadata
    let (returned_habit, sessions) =
        extract_nft_metadata(&btc_client, &nft_utxo.txid.to_string()).expect("extract metadata");

    assert_eq!(returned_habit, habit_name, "Habit name should match");
    assert_eq!(sessions, 0, "Initial sessions should be 0");

    println!("✅ CLI create test passed!");
    println!("   NFT UTXO: {}", nft_id);
    println!("   Habit: {}", returned_habit);
    println!("   Sessions: {}", sessions);
}

#[tokio::test] 
#[serial]
async fn cli_update_nft_works() {
    println!("\n🧪 Testing CLI: update command\n");

    let node = get_or_init_bitcoin_node();
    let btc_client = get_bitcoincore_rpc_client(node).expect("create client");

    // Fund wallet
    let user_addr = btc_client
        .get_new_address(None, None)
        .expect("get new address")
        .require_network(bitcoin::Network::Regtest)
        .expect("check network");

    println!("⛏️  Generating blocks for wallet funds...");
    node.client
        .generate_to_address(101, &user_addr)
        .expect("generate blocks");

    // Create initial NFT with unique name
    println!("📝 Creating initial NFT...");
    let habit_name = "CLI Update Test".to_string();
    create_nft(&btc_client, habit_name.clone()).expect("create NFT");

    // Mine to confirm
    let mining_addr = node
        .client
        .get_new_address(None, None)
        .expect("get mining address")
        .into_model()
        .expect("convert address")
        .0
        .assume_checked();

    node.client
        .generate_to_address(1, &mining_addr)
        .expect("mine block");

    std::thread::sleep(std::time::Duration::from_millis(500));

    // Find NFT UTXO by habit name (NEW APPROACH)
    let utxos = btc_client
        .list_unspent(None, None, None, None, None)
        .expect("list unspent");

    let nft_utxos: Vec<_> = utxos.iter().filter(|u| u.amount.to_sat() == 1000).collect();

    println!("   Found {} NFT UTXOs total", nft_utxos.len());

    // Find the one with our specific habit name
    let nft_utxo = nft_utxos
        .iter()
        .find(|utxo| {
            if let Ok((habit, _)) = extract_nft_metadata(&btc_client, &utxo.txid.to_string()) {
                habit == habit_name
            } else {
                false
            }
        })
        .expect("Should find NFT with correct habit name");

    let nft_utxo_id = format!("{}:{}", nft_utxo.txid, nft_utxo.vout);
    println!("   Found NFT at: {}", nft_utxo_id);

    // Verify initial state
    let (_, initial_sessions) =
        extract_nft_metadata(&btc_client, &nft_utxo.txid.to_string()).expect("extract metadata");
    assert_eq!(initial_sessions, 0, "Should start with 0 sessions");

    // Update via CLI
    println!("🔄 Updating NFT via CLI...");
    let result = update_nft(&btc_client, nft_utxo_id.clone()).await;

    assert!(
        result.is_ok(),
        "CLI update_nft should succeed: {:?}",
        result.err()
    );

    // Mine to confirm update
    node.client
        .generate_to_address(1, &mining_addr)
        .expect("mine block");

    std::thread::sleep(std::time::Duration::from_millis(500));

    // Find new NFT UTXO by habit name AND session count (NEW APPROACH)
    let new_utxos = btc_client
        .list_unspent(None, None, None, None, None)
        .expect("list unspent");

    let new_nft_utxos: Vec<_> = new_utxos
        .iter()
        .filter(|u| u.amount.to_sat() == 1000)
        .collect();

    println!("   Found {} NFT UTXOs after update", new_nft_utxos.len());

    // Find the updated NFT (same habit name, but sessions = 1)
    let new_nft_utxo = new_nft_utxos
        .iter()
        .find(|utxo| {
            if let Ok((habit, sessions)) = extract_nft_metadata(&btc_client, &utxo.txid.to_string())
            {
                habit == habit_name && sessions == 1
            } else {
                false
            }
        })
        .expect("Should find updated NFT with 1 session");

    let new_nft_id = format!("{}:{}", new_nft_utxo.txid, new_nft_utxo.vout);

    // Verify updated state
    let (_, updated_sessions) = extract_nft_metadata(&btc_client, &new_nft_utxo.txid.to_string())
        .expect("extract metadata");

    assert_eq!(updated_sessions, 1, "Sessions should be incremented to 1");
    assert_ne!(
        nft_utxo_id, new_nft_id,
        "UTXO should be different after update"
    );

    println!("✅ CLI update test passed!");
    println!("   Old UTXO: {}", nft_utxo_id);
    println!("   New UTXO: {}", new_nft_id);
    println!("   Sessions: {} → {}", initial_sessions, updated_sessions);
}

#[test]
#[serial]
fn cli_view_nft_works() {
    println!("\n🧪 Testing CLI: view command\n");

    let node = get_or_init_bitcoin_node();
    let btc_client = get_bitcoincore_rpc_client(node).expect("create client");

    // Fund wallet
    let user_addr = btc_client
        .get_new_address(None, None)
        .expect("get new address")
        .require_network(bitcoin::Network::Regtest)
        .expect("check network");

    println!("⛏️  Generating blocks for wallet funds...");
    node.client
        .generate_to_address(101, &user_addr)
        .expect("generate blocks");

    // Create NFT with unique habit name
    let habit_name = "CLI View Test Habit".to_string();
    println!("📝 Creating NFT with habit: {}", habit_name);

    create_nft(&btc_client, habit_name.clone()).expect("create NFT");

    // Mine to confirm
    let mining_addr = node
        .client
        .get_new_address(None, None)
        .expect("get mining address")
        .into_model()
        .expect("convert address")
        .0
        .assume_checked();

    node.client
        .generate_to_address(1, &mining_addr)
        .expect("mine block");

    std::thread::sleep(std::time::Duration::from_millis(500));

    // Find NFT UTXO by habit name (NEW APPROACH)
    let utxos = btc_client
        .list_unspent(None, None, None, None, None)
        .expect("list unspent");

    let nft_utxos: Vec<_> = utxos.iter().filter(|u| u.amount.to_sat() == 1000).collect();

    println!("   Found {} NFT UTXOs total", nft_utxos.len());

    // Find the one with our specific habit name
    let nft_utxo = nft_utxos
        .iter()
        .find(|utxo| {
            if let Ok((habit, _)) = extract_nft_metadata(&btc_client, &utxo.txid.to_string()) {
                habit == habit_name
            } else {
                false
            }
        })
        .expect("Should find NFT with correct habit name");

    let nft_utxo_id = format!("{}:{}", nft_utxo.txid, nft_utxo.vout);
    println!("   Using NFT: {}", nft_utxo_id);

    // View via CLI
    println!("👀 Viewing NFT via CLI...");
    let result = view_nft(&btc_client, nft_utxo_id.clone());

    assert!(
        result.is_ok(),
        "CLI view_nft should succeed: {:?}",
        result.err()
    );

    // Verify metadata
    let (viewed_habit, sessions) =
        extract_nft_metadata(&btc_client, &nft_utxo.txid.to_string()).expect("extract metadata");

    assert_eq!(viewed_habit, habit_name, "Habit name should match");
    assert_eq!(sessions, 0, "Sessions should be 0");

    println!("✅ CLI view test passed!");
    println!("   UTXO: {}", nft_utxo_id);
    println!("   Habit: {}", viewed_habit);
    println!("   Sessions: {}", sessions);
}
