use crate::*;
use serde_json::{json, Value};
use serial_test::serial;
use std::str::FromStr;
use std::{env, time::SystemTime};

use bitcoincore_rpc::{bitcoin, bitcoin::Txid, Auth, Client as BitcoinCoreClient, RpcApi};
use corepc_node::{Conf, Node};

fn unique_habit_name(base: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{} {}", base, timestamp)
}

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

    let wallet_name = format!(
        "test_{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Try simple create wallet first
    match base_client.create_wallet(&wallet_name, None, None, None, None) {
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
            let _ = base_client.load_wallet(&wallet_name);

            let wallet_url = format!("http://{}/wallet/{}", params.rpc_socket, wallet_name);
            let wallet_client = BitcoinCoreClient::new(
                &wallet_url,
                Auth::UserPass(cookie_values.user, cookie_values.password),
            )?;

            Ok(wallet_client)
        }
    }
}

fn _print_spell(client: &bitcoincore_rpc::Client, txid: &Txid) -> anyhow::Result<()> {
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

// ============================================================================
// Test Helpers
// ============================================================================

struct TestBitcoin {
    _node: Node,
    client: BitcoinCoreClient,
}

fn setup_test_bitcoin() -> anyhow::Result<TestBitcoin> {
    if env::var("TEMPDIR_ROOT").is_err() {
        env::set_var("TEMPDIR_ROOT", "/dev/shm");
    }

    // Require CHARMS_BIN to be set for tests
    env::var("CHARMS_BIN").expect(
        "CHARMS_BIN environment variable must be set for tests.\n\
         Set it with: export CHARMS_BIN=/path/to/charms\n\
        "
    );

    let mut conf = Conf::default();
    conf.args = vec!["-regtest", "-fallbackfee=0.0001", "-txindex=1"];
    conf.tmpdir = None;

    let node = Node::from_downloaded_with_conf(&conf)?;
    let client = get_bitcoincore_rpc_client(&node)?;

    let mining_addr = client
        .get_new_address(None, None)?
        .require_network(bitcoin::Network::Regtest)?;

    client.generate_to_address(101, &mining_addr)?;

    Ok(TestBitcoin {
        _node: node,
        client,
    })
}

impl TestBitcoin {
    fn mine_block(&self) -> anyhow::Result<()> {
        let addr = self
            .client
            .get_new_address(None, None)?
            .require_network(bitcoin::Network::Regtest)?;

        self.client.generate_to_address(1, &addr)?;
        Ok(())
    }

    fn get_new_address(&self) -> anyhow::Result<bitcoin::Address> {
        self.client
            .get_new_address(None, None)?
            .require_network(bitcoin::Network::Regtest)
            .map_err(Into::into)
    }

    fn get_first_utxo(&self) -> anyhow::Result<bitcoincore_rpc::json::ListUnspentResultEntry> {
        let utxos = self.client.list_unspent(None, None, None, None, None)?;
        utxos
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no UTXOs available"))
    }

    fn get_funding_utxo(&self) -> anyhow::Result<bitcoincore_rpc::json::ListUnspentResultEntry> {
        self.client
            .list_unspent(None, None, None, None, None)?
            .into_iter()
            .find(|u| u.amount.to_sat() != 1000)
            .ok_or_else(|| anyhow::anyhow!("no funding UTXO available"))
    }

    fn find_nft_utxo(&self) -> anyhow::Result<bitcoincore_rpc::json::ListUnspentResultEntry> {
        self.client
            .list_unspent(None, None, None, None, None)?
            .into_iter()
            .find(|u| u.amount.to_sat() == 1000)
            .ok_or_else(|| anyhow::anyhow!("NFT UTXO not found"))
    }

    fn find_nft_by_txid(
        &self,
        txid: &str,
    ) -> anyhow::Result<bitcoincore_rpc::json::ListUnspentResultEntry> {
        self.client
            .list_unspent(None, None, None, None, None)?
            .into_iter()
            .find(|u| u.txid.to_string() == txid && u.vout == 0)
            .ok_or_else(|| anyhow::anyhow!("NFT with txid {} not found", txid))
    }
}

struct SignedTransactions {
    commit_hex: String,
    spell_hex: String,
}

fn sign_transactions(
    client: &BitcoinCoreClient,
    commit_hex: &str,
    spell_hex: &str,
    nft_utxo: Option<&bitcoincore_rpc::json::ListUnspentResultEntry>,
) -> anyhow::Result<SignedTransactions> {
    let commit_tx: bitcoin::Transaction =
        bitcoin::consensus::deserialize(&hex::decode(commit_hex)?)?;
    let spell_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&hex::decode(spell_hex)?)?;

    let signed_commit = client.sign_raw_transaction_with_wallet(&commit_tx, None, None)?;
    assert!(signed_commit.complete, "commit tx signing incomplete");

    let mut prevouts = vec![bitcoincore_rpc::json::SignRawTransactionInput {
        txid: commit_tx.compute_txid(),
        vout: 0,
        script_pub_key: commit_tx.output[0].script_pubkey.clone(),
        redeem_script: None,
        amount: Some(commit_tx.output[0].value),
    }];

    // Add NFT prevout if this is an update
    if let Some(nft) = nft_utxo {
        let nft_tx = client.get_raw_transaction(&nft.txid, None)?;
        prevouts.push(bitcoincore_rpc::json::SignRawTransactionInput {
            txid: nft.txid,
            vout: nft.vout,
            script_pub_key: nft_tx.output[nft.vout as usize].script_pubkey.clone(),
            redeem_script: None,
            amount: Some(bitcoin::Amount::from_sat(1000)),
        });
    }

    let signed_spell = client.sign_raw_transaction_with_wallet(&spell_tx, Some(&prevouts), None)?;
    assert!(signed_spell.complete, "spell tx signing incomplete");

    Ok(SignedTransactions {
        commit_hex: hex::encode(&signed_commit.hex),
        spell_hex: hex::encode(&signed_spell.hex),
    })
}

fn verify_spell_has_charms(client: &BitcoinCoreClient, txid: &Txid) -> anyhow::Result<()> {
    let tx_hex = client.get_raw_transaction_hex(txid, None)?;

    let output = std::process::Command::new("charms")
        .args(&["tx", "show-spell", "--tx", &tx_hex, "--mock", "--json"])
        .output()?;

    assert!(output.status.success(), "charms decode failed");

    let spell: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    let has_charms = spell
        .get("outs")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|o| o.get("charms").is_some()))
        .unwrap_or(false);

    assert!(has_charms, "spell must contain charms");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[test]
#[serial]
fn create_nft_works() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");
    let user_addr = bitcoin.get_new_address().expect("get address");
    let funding_utxo = bitcoin.get_first_utxo().expect("get funding utxo");

    // Create unsigned transactions
    let habit_name = unique_habit_name("Morning Meditation");
    let unsigned = create_nft_unsigned(
        habit_name,
        user_addr.to_string(),
        format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
        funding_utxo.amount.to_sat(),
    )
    .expect("create unsigned");

    assert!(!unsigned.commit_tx_hex.is_empty());
    assert!(!unsigned.spell_tx_hex.is_empty());
    assert!(!unsigned.commit_txid.is_empty());
    assert_eq!(unsigned.spell_inputs_info.len(), 2);

    // Sign transactions (no NFT for create)
    let signed = sign_transactions(
        &bitcoin.client,
        &unsigned.commit_tx_hex,
        &unsigned.spell_tx_hex,
        None,
    )
    .expect("sign transactions");

    // Broadcast
    let broadcast =
        broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex).expect("broadcast");

    // Confirm
    bitcoin.mine_block().expect("mine block");

    // Verify NFT was created
    let nft_utxo = bitcoin.find_nft_utxo().expect("find NFT");
    assert_eq!(nft_utxo.txid.to_string(), broadcast.spell_txid);
    assert_eq!(nft_utxo.amount.to_sat(), 1000);

    verify_spell_has_charms(&bitcoin.client, &nft_utxo.txid).expect("verify spell");
}

#[test]
#[serial]
fn update_nft_works() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    // Create initial NFT
    let habit_name = unique_habit_name("Update Test");
    let nft_txid = create_nft(&bitcoin.client, habit_name).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    // Get NFT and funding UTXOs
    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    let funding_utxo = bitcoin.get_funding_utxo().expect("get funding");

    // we need the same address so owner does not change
    let (_habit, _sessions, owner_addr) =
        extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");

    // Create unsigned update transactions
    let unsigned = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:{}", nft_utxo.txid, nft_utxo.vout),
        owner_addr.to_string(),
        format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
        funding_utxo.amount.to_sat(),
    )
    .expect("create unsigned update");

    assert_eq!(unsigned.current_sessions, 0);
    assert_eq!(unsigned.new_sessions, 1);
    assert_eq!(unsigned.spell_inputs_info.len(), 3);

    // Sign transactions (with NFT for update)
    let signed = sign_transactions(
        &bitcoin.client,
        &unsigned.commit_tx_hex,
        &unsigned.spell_tx_hex,
        Some(&nft_utxo),
    )
    .expect("sign transactions");

    // Broadcast
    let broadcast =
        broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex).expect("broadcast");

    // Confirm
    bitcoin.mine_block().expect("mine block");

    // Verify NFT was updated
    let updated_nft = bitcoin
        .find_nft_by_txid(&broadcast.spell_txid)
        .expect("find updated NFT");
    assert_eq!(updated_nft.amount.to_sat(), 1000);

    let (_, sessions, _habit_name) =
        extract_nft_metadata(&bitcoin.client, &broadcast.spell_txid).expect("extract metadata");
    assert_eq!(sessions, 1);
}

#[test]
#[serial]
fn cli_create_nft_works() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    let habit_name = unique_habit_name("CLI Test Habit");
    let nft_txid = create_nft(&bitcoin.client, habit_name.clone()).expect("create NFT");

    bitcoin.mine_block().expect("mine block");

    // Verify NFT exists with correct metadata
    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    assert_eq!(nft_utxo.amount.to_sat(), 1000);

    let (returned_habit, sessions, _) =
        extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");

    assert_eq!(returned_habit, habit_name);
    assert_eq!(sessions, 0);
}

#[tokio::test]
#[serial]
async fn cli_update_nft_works() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    // Create initial NFT
    let habit_name = unique_habit_name("CLI Update Test");
    let nft_txid = create_nft(&bitcoin.client, habit_name.clone()).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    let nft_utxo_id = format!("{}:{}", nft_utxo.txid, nft_utxo.vout);

    // Verify initial state
    let (_, initial_sessions, _) =
        extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");
    assert_eq!(initial_sessions, 0);

    // Update via CLI
    update_nft(&bitcoin.client, nft_utxo_id.clone())
        .await
        .expect("update NFT");
    bitcoin.mine_block().expect("mine block");

    // Verify updated NFT
    let updated_nft = bitcoin.find_nft_utxo().expect("find updated NFT");
    let (returned_habit, updated_sessions, _) =
        extract_nft_metadata(&bitcoin.client, &updated_nft.txid.to_string())
            .expect("extract metadata");

    assert_eq!(returned_habit, habit_name);
    assert_eq!(updated_sessions, 1);
    assert_ne!(updated_nft.txid.to_string(), nft_txid);
}

#[test]
#[serial]
fn cli_view_nft_works() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    let habit_name = unique_habit_name("CLI View Test");
    let nft_txid = create_nft(&bitcoin.client, habit_name.clone()).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    let nft_utxo_id = format!("{}:{}", nft_utxo.txid, nft_utxo.vout);

    // View via CLI
    view_nft(&bitcoin.client, nft_utxo_id).expect("view NFT");

    // Verify metadata
    let (viewed_habit, sessions, _) =
        extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");

    assert_eq!(viewed_habit, habit_name);
    assert_eq!(sessions, 0);
}

#[test]
#[serial]
fn app_preserves_owner_on_update() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    // Create NFT
    let habit_name = unique_habit_name("Owner Preservation Test");
    let nft_txid = create_nft(&bitcoin.client, habit_name).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    let (_, _, original_owner) =
        extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");

    // Update NFT
    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    let funding_utxo = bitcoin.get_funding_utxo().expect("get funding");

    let unsigned = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:0", nft_txid),
        original_owner.clone(), // Use same owner
        format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
        funding_utxo.amount.to_sat(),
    )
    .expect("create unsigned update");

    let signed = sign_transactions(
        &bitcoin.client,
        &unsigned.commit_tx_hex,
        &unsigned.spell_tx_hex,
        Some(&nft_utxo),
    )
    .expect("sign transactions");

    let broadcast =
        broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex).expect("broadcast");

    bitcoin.mine_block().expect("mine block");

    // Verify owner is preserved
    let (_, _, new_owner) =
        extract_nft_metadata(&bitcoin.client, &broadcast.spell_txid).expect("extract metadata");

    assert_eq!(
        original_owner, new_owner,
        "App must preserve owner on update"
    );
}

#[test]
#[serial]
fn app_increments_sessions_correctly() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    // Create NFT
    let habit_name = unique_habit_name("Session Increment Test");
    let nft_txid = create_nft(&bitcoin.client, habit_name).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    // Verify starts at 0
    let (_, sessions_0, owner) =
        extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");
    assert_eq!(sessions_0, 0);

    // Update 1
    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    let funding_utxo = bitcoin.get_funding_utxo().expect("get funding");

    let unsigned = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:0", nft_txid),
        owner.clone(),
        format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
        funding_utxo.amount.to_sat(),
    )
    .expect("create unsigned update");

    assert_eq!(unsigned.current_sessions, 0);
    assert_eq!(unsigned.new_sessions, 1);

    let signed = sign_transactions(
        &bitcoin.client,
        &unsigned.commit_tx_hex,
        &unsigned.spell_tx_hex,
        Some(&nft_utxo),
    )
    .expect("sign transactions");

    let broadcast =
        broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex).expect("broadcast");

    bitcoin.mine_block().expect("mine block");

    // Verify incremented to 1
    let (_, sessions_1, _) =
        extract_nft_metadata(&bitcoin.client, &broadcast.spell_txid).expect("extract metadata");
    assert_eq!(sessions_1, 1);
}

#[test]
#[serial]
fn app_assigns_correct_badges() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    // Create NFT (0 sessions = no badges)
    let habit_name = unique_habit_name("Badge Test");
    let nft_txid = create_nft(&bitcoin.client, habit_name).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    let tx_hex_0 = bitcoin
        .client
        .get_raw_transaction_hex(&bitcoin::Txid::from_str(&nft_txid).unwrap(), None)
        .unwrap();

    let spell_output_0 = std::process::Command::new("charms")
        .args(&["tx", "show-spell", "--tx", &tx_hex_0, "--mock", "--json"])
        .output()
        .unwrap();

    let spell_0: serde_json::Value = serde_json::from_slice(&spell_output_0.stdout).unwrap();

    // For 0 sessions, badges field might be missing or empty
    let badges_0 = spell_0
        .get("outs")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|out| out.get("charms"))
        .and_then(|charms| charms.get("$0000"))
        .and_then(|charm| charm.get("badges"))
        .and_then(|b| b.as_array());

    // Badges field might be missing for empty array
    if let Some(badges) = badges_0 {
        assert_eq!(badges.len(), 0, "0 sessions should have no badges");
    } else {
        // Missing badges field is OK for 0 sessions
        println!("Badges field omitted for empty array (expected for 0 sessions)");
    }

    // Update to session 1
    let (_, _, owner) = extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");
    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    let funding_utxo = bitcoin.get_funding_utxo().expect("get funding");

    let unsigned = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:0", nft_txid),
        owner,
        format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
        funding_utxo.amount.to_sat(),
    )
    .expect("create unsigned update");

    let signed = sign_transactions(
        &bitcoin.client,
        &unsigned.commit_tx_hex,
        &unsigned.spell_tx_hex,
        Some(&nft_utxo),
    )
    .expect("sign transactions");

    let broadcast =
        broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex).expect("broadcast");

    bitcoin.mine_block().expect("mine block");

    // Verify "First Strike" badge at session 1
    let tx_hex_1 = bitcoin
        .client
        .get_raw_transaction_hex(
            &bitcoin::Txid::from_str(&broadcast.spell_txid).unwrap(),
            None,
        )
        .unwrap();

    let spell_output_1 = std::process::Command::new("charms")
        .args(&["tx", "show-spell", "--tx", &tx_hex_1, "--mock", "--json"])
        .output()
        .unwrap();

    let spell_1: serde_json::Value = serde_json::from_slice(&spell_output_1.stdout).unwrap();

    let badges_1 = spell_1
        .get("outs")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|out| out.get("charms"))
        .and_then(|charms| charms.get("$0000"))
        .and_then(|charm| charm.get("badges"))
        .and_then(|b| b.as_array())
        .expect("Session 1 should have badges field");

    assert_eq!(badges_1.len(), 1, "Session 1 should have 1 badge");
    assert_eq!(badges_1[0].as_str().unwrap(), "🌸 First Blood");
}

#[test]
#[serial]
fn app_extracts_metadata_correctly() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    let habit_name = unique_habit_name("Metadata Test");

    let nft_txid = create_nft(&bitcoin.client, habit_name.clone()).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    let (extracted_habit, sessions, owner) =
        extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");

    assert_eq!(extracted_habit, habit_name);
    assert_eq!(sessions, 0);
    assert!(!owner.is_empty());
}

#[test]
#[serial]
fn app_handles_multiple_updates() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    let habit_name = unique_habit_name("Multiple Updates Test");
    let mut current_txid = create_nft(&bitcoin.client, habit_name).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    // First update doesn't need to wait (no last_updated in input)
    let (_, _, owner) =
        extract_nft_metadata(&bitcoin.client, &current_txid).expect("extract metadata");

    let nft_utxo = bitcoin.find_nft_by_txid(&current_txid).expect("find NFT");
    let funding_utxo = bitcoin.get_funding_utxo().expect("get funding");

    let unsigned = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:0", current_txid),
        owner.clone(),
        format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
        funding_utxo.amount.to_sat(),
    )
    .expect("create unsigned update");

    let signed = sign_transactions(
        &bitcoin.client,
        &unsigned.commit_tx_hex,
        &unsigned.spell_tx_hex,
        Some(&nft_utxo),
    )
    .expect("sign transactions");

    let broadcast =
        broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex).expect("broadcast");

    bitcoin.mine_block().expect("mine block");
    current_txid = broadcast.spell_txid;

    // Subsequent updates need to wait 5 seconds
    for expected_session in 2..=3 {
        println!(
            "Waiting 5 seconds before update to session {}...",
            expected_session
        );
        std::thread::sleep(std::time::Duration::from_secs(5));

        let nft_utxo = bitcoin.find_nft_by_txid(&current_txid).expect("find NFT");
        let funding_utxo = bitcoin.get_funding_utxo().expect("get funding");

        let unsigned = update_nft_unsigned(
            &bitcoin.client,
            format!("{}:0", current_txid),
            owner.clone(),
            format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
            funding_utxo.amount.to_sat(),
        )
        .expect("create unsigned update");

        let signed = sign_transactions(
            &bitcoin.client,
            &unsigned.commit_tx_hex,
            &unsigned.spell_tx_hex,
            Some(&nft_utxo),
        )
        .expect("sign transactions");

        let broadcast =
            broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex).expect("broadcast");

        bitcoin.mine_block().expect("mine block");

        let (_, sessions, _) =
            extract_nft_metadata(&bitcoin.client, &broadcast.spell_txid).expect("extract metadata");
        assert_eq!(sessions, expected_session);

        current_txid = broadcast.spell_txid;
    }
}

#[test]
#[serial]
fn contract_enforces_time_restriction() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    // Create NFT and do first update
    let habit_name = unique_habit_name("Time Restriction Test");
    let nft_txid = create_nft(&bitcoin.client, habit_name).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    let (_, _, owner) = extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");
    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    let funding_utxo = bitcoin.get_funding_utxo().expect("get funding");

    // First update (should work - no previous timestamp)
    let unsigned = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:0", nft_txid),
        owner.clone(),
        format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
        funding_utxo.amount.to_sat(),
    )
    .expect("create unsigned update");

    let signed = sign_transactions(
        &bitcoin.client,
        &unsigned.commit_tx_hex,
        &unsigned.spell_tx_hex,
        Some(&nft_utxo),
    )
    .expect("sign transactions");
    let broadcast = broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex)
        .expect("first update should succeed");
    bitcoin.mine_block().expect("mine block");

    // Try to update immediately (should FAIL)
    let _nft_utxo_2 = bitcoin
        .find_nft_by_txid(&broadcast.spell_txid)
        .expect("find NFT");
    let funding_utxo_2 = bitcoin.get_funding_utxo().expect("get funding");

    let result = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:0", broadcast.spell_txid),
        owner,
        format!("{}:{}", funding_utxo_2.txid, funding_utxo_2.vout),
        funding_utxo_2.amount.to_sat(),
    );

    assert!(result.is_err(), "Update should fail when done too soon");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Update too soon") || err_msg.contains("Must wait 5 seconds"),
        "Error should mention time restriction. Got: {}",
        err_msg
    );
}

#[test]
#[serial]
fn contract_allows_update_after_waiting() {
    let contract_path = get_contract_path();
    assert!(
        contract_path.exists(),
        "Contract WASM required. Run: make contract"
    );

    let bitcoin = setup_test_bitcoin().expect("setup bitcoin");

    // Create NFT and do first update
    let habit_name = unique_habit_name("Wait Time Test");
    let nft_txid = create_nft(&bitcoin.client, habit_name).expect("create NFT");
    bitcoin.mine_block().expect("mine block");

    let (_, _, owner) = extract_nft_metadata(&bitcoin.client, &nft_txid).expect("extract metadata");
    let nft_utxo = bitcoin.find_nft_by_txid(&nft_txid).expect("find NFT");
    let funding_utxo = bitcoin.get_funding_utxo().expect("get funding");

    // First update
    let unsigned = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:0", nft_txid),
        owner.clone(),
        format!("{}:{}", funding_utxo.txid, funding_utxo.vout),
        funding_utxo.amount.to_sat(),
    )
    .expect("create unsigned update");

    let signed = sign_transactions(
        &bitcoin.client,
        &unsigned.commit_tx_hex,
        &unsigned.spell_tx_hex,
        Some(&nft_utxo),
    )
    .expect("sign transactions");
    let broadcast = broadcast_nft(&bitcoin.client, signed.commit_hex, signed.spell_hex)
        .expect("first update should succeed");
    bitcoin.mine_block().expect("mine block");

    // Wait 5 seconds
    println!("Waiting 5 seconds for time restriction...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    // Try to update after waiting (should SUCCEED)
    let nft_utxo_2 = bitcoin
        .find_nft_by_txid(&broadcast.spell_txid)
        .expect("find NFT");
    let funding_utxo_2 = bitcoin.get_funding_utxo().expect("get funding");

    let unsigned_2 = update_nft_unsigned(
        &bitcoin.client,
        format!("{}:0", broadcast.spell_txid),
        owner,
        format!("{}:{}", funding_utxo_2.txid, funding_utxo_2.vout),
        funding_utxo_2.amount.to_sat(),
    )
    .expect("update should succeed after waiting");

    let signed_2 = sign_transactions(
        &bitcoin.client,
        &unsigned_2.commit_tx_hex,
        &unsigned_2.spell_tx_hex,
        Some(&nft_utxo_2),
    )
    .expect("sign transactions");
    let broadcast_2 = broadcast_nft(&bitcoin.client, signed_2.commit_hex, signed_2.spell_hex)
        .expect("second update should succeed after waiting");
    bitcoin.mine_block().expect("mine block");

    // Verify we got to session 2
    let (_, sessions, _) =
        extract_nft_metadata(&bitcoin.client, &broadcast_2.spell_txid).expect("extract metadata");
    assert_eq!(sessions, 2, "Should have 2 sessions after second update");
}
