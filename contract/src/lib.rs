use charms_sdk::data::{charm_values, check, App, Data, Transaction, NFT};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HabitContent {
    pub name: String,
    pub description: String,
    pub owner: String,
    pub habit_name: String,
    pub total_sessions: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<i64>,
    #[serde(default)]
    pub badges: Vec<String>,
}

// Configurable time window for testing (in seconds)
// Production: 86400 (24 hours)
// Testing: 5 (5 seconds for fast testing)
const MIN_UPDATE_INTERVAL_SECS: i64 = 5;

pub fn app_contract(app: &App, tx: &Transaction, x: &Data, w: &Data) -> bool {
    let empty = Data::empty();
    assert_eq!(x, &empty);
    
    // Note: `w` is witness data used for proof-of-burn or other validation.
    // The habit tracker doesn't need it - we validate based on NFT content only.
    // We still accept it because this is the required contract interface.
    let _ = w; // mark as unused
    
    match app.tag {
        NFT => {
            check!(nft_contract_satisfied(app, tx))
        }
        _ => unreachable!(),
    }
    true
}

// Main NFT validation logic
fn nft_contract_satisfied(app: &App, tx: &Transaction) -> bool {
    // Extract input NFT (if exists - creation has no inputs)
    let input_nft: Option<HabitContent> =
        charm_values(app, tx.ins.iter().map(|(_, v)| v)).find_map(|data| data.value().ok());

    // Extract output NFT
    let output_nft: Option<HabitContent> =
        charm_values(app, tx.outs.iter()).find_map(|data| data.value().ok());

    check!(output_nft.is_some());
    let output = output_nft.unwrap();

    // Call the pure validation logic
    check!(validate_habit_logic(input_nft, output));
    true
}

// Pure validation logic - can be tested directly
pub(crate) fn validate_habit_logic(
    input_nft: Option<HabitContent>,
    output: HabitContent,
) -> bool {
    // If no input NFT, this is creation - allow it
    if input_nft.is_none() {
        eprintln!("✓ NFT creation - basic validation passed");
        return true;
    }

    let input = input_nft.unwrap();

    // Rule 1: Owner must not change
    if input.owner != output.owner {
        eprintln!("✗ Owner cannot be changed");
        return false;
    }

    // Rule 2: Sessions must increment by exactly 1
    if output.total_sessions != input.total_sessions + 1 {
        eprintln!(
            "✗ Sessions must increment by 1 (was: {}, now: {})",
            input.total_sessions, output.total_sessions
        );
        return false;
    }

    // Rule 3: Time restriction - must wait MIN_UPDATE_INTERVAL_SECS between updates
    if let (Some(last), Some(now)) = (input.last_updated, output.last_updated) {
        let elapsed = now - last;
        if elapsed < MIN_UPDATE_INTERVAL_SECS {
            eprintln!(
                "✗ Update too soon. Must wait {} seconds, only {} elapsed",
                MIN_UPDATE_INTERVAL_SECS, elapsed
            );
            return false;
        }
    }

    // Rule 4: Validate badges are correct for session count
    let expected_badges = get_badges_for_sessions(output.total_sessions);
    if output.badges != expected_badges {
        eprintln!(
            "✗ Badge mismatch. Expected: {:?}, Got: {:?}",
            expected_badges, output.badges
        );
        return false;
    }

    eprintln!(
        "✓ Update validated: {} → {} sessions, badges: {:?}",
        input.total_sessions, output.total_sessions, output.badges
    );
    true
}

// Badge system - The Samurai Path to Mastery (66 Days)
// Based on neuroscience (Robin Sharma) + Bushido philosophy
fn get_badges_for_sessions(sessions: u64) -> Vec<String> {
    let mut badges = Vec::new();

    // Stage 1: DESTRUCTION (Days 1-22) - Breaking Old Patterns
    if sessions >= 1 {
        badges.push("🌸 First Blood".to_string());
    }
    if sessions >= 3 {
        badges.push("⚔️ Three Cuts".to_string());
    }
    if sessions >= 7 {
        badges.push("🔥 Week Warrior".to_string());
    }
    if sessions >= 11 {
        badges.push("🌊 Rising Tide".to_string());
    }
    if sessions >= 15 {
        badges.push("⛩️ Temple Guardian".to_string());
    }
    if sessions >= 22 {
        badges.push("💥 Destruction Complete".to_string());
    }

    // Stage 2: INSTALLATION (Days 23-44) - Forging the New Way
    if sessions >= 23 {
        badges.push("🔨 The Forge Begins".to_string());
    }
    if sessions >= 30 {
        badges.push("🗡️ Month of Steel".to_string());
    }
    if sessions >= 33 {
        badges.push("⚡ Thunder Strike".to_string());
    }
    if sessions >= 40 {
        badges.push("🌙 Moonlit Path".to_string());
    }
    if sessions >= 44 {
        badges.push("🎌 Installation Complete".to_string());
    }

    // Stage 3: INTEGRATION (Days 45-66) - Becoming the Master
    if sessions >= 45 {
        badges.push("🌅 Dawn of Mastery".to_string());
    }
    if sessions >= 50 {
        badges.push("🏔️ Mountain Summit".to_string());
    }
    if sessions >= 55 {
        badges.push("🐉 Dragon Awakens".to_string());
    }
    if sessions >= 60 {
        badges.push("⭐ Celestial Alignment".to_string());
    }
    if sessions >= 66 {
        badges.push("👑 Shogun".to_string());
    }

    // Beyond Mastery (Legendary Tier)
    if sessions >= 100 {
        badges.push("💯 Century Samurai".to_string());
    }
    if sessions >= 200 {
        badges.push("🌸⚔️ Twin Blades".to_string());
    }
    if sessions >= 365 {
        badges.push("🏯 Daimyo".to_string());
    }
    if sessions >= 500 {
        badges.push("🔮 Mystic Warrior".to_string());
    }
    if sessions >= 1000 {
        badges.push("⛩️👑 Living Legend".to_string());
    }

    badges
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_badge_progression() {
        // Test no badges at start
        assert_eq!(get_badges_for_sessions(0), Vec::<String>::new());
        
        // Stage 1: DESTRUCTION
        assert_eq!(get_badges_for_sessions(1), vec!["🌸 First Blood"]);
        assert_eq!(
            get_badges_for_sessions(3),
            vec!["🌸 First Blood", "⚔️ Three Cuts"]
        );
        assert_eq!(
            get_badges_for_sessions(7),
            vec!["🌸 First Blood", "⚔️ Three Cuts", "🔥 Week Warrior"]
        );
        assert_eq!(
            get_badges_for_sessions(22),
            vec![
                "🌸 First Blood",
                "⚔️ Three Cuts",
                "🔥 Week Warrior",
                "🌊 Rising Tide",
                "⛩️ Temple Guardian",
                "💥 Destruction Complete"
            ]
        );
        
        // Stage 2: INSTALLATION
        assert_eq!(
            get_badges_for_sessions(23),
            vec![
                "🌸 First Blood",
                "⚔️ Three Cuts",
                "🔥 Week Warrior",
                "🌊 Rising Tide",
                "⛩️ Temple Guardian",
                "💥 Destruction Complete",
                "🔨 The Forge Begins"
            ]
        );
        assert_eq!(
            get_badges_for_sessions(30),
            vec![
                "🌸 First Blood",
                "⚔️ Three Cuts",
                "🔥 Week Warrior",
                "🌊 Rising Tide",
                "⛩️ Temple Guardian",
                "💥 Destruction Complete",
                "🔨 The Forge Begins",
                "🗡️ Month of Steel"
            ]
        );
        assert_eq!(
            get_badges_for_sessions(44).len(),
            11 // All Destruction + All Installation badges
        );
        
        // Stage 3: INTEGRATION
        assert_eq!(
            get_badges_for_sessions(45).len(),
            12 // Previous + Dawn of Mastery
        );
        assert_eq!(
            get_badges_for_sessions(66).len(),
            16 // All 3 stages complete
        );
        
        // Legendary Tier
        assert_eq!(
            get_badges_for_sessions(100).len(),
            17 // All stages + Century Samurai
        );
        assert_eq!(
            get_badges_for_sessions(365).len(),
            19 // + Twin Blades + Daimyo
        );
        assert_eq!(
            get_badges_for_sessions(1000).len(),
            21 // All 21 badges
        );
    }

    #[test]
    fn test_stage_milestones() {
        // Test key milestones for each stage
        
        // Destruction Complete (end of Stage 1)
        let badges_22 = get_badges_for_sessions(22);
        assert!(badges_22.contains(&"💥 Destruction Complete".to_string()));
        assert_eq!(badges_22.len(), 6);
        
        // Installation Complete (end of Stage 2)
        let badges_44 = get_badges_for_sessions(44);
        assert!(badges_44.contains(&"🎌 Installation Complete".to_string()));
        assert_eq!(badges_44.len(), 11);
        
        // Shogun (mastery achieved - end of Stage 3)
        let badges_66 = get_badges_for_sessions(66);
        assert!(badges_66.contains(&"👑 Shogun".to_string()));
        assert_eq!(badges_66.len(), 16);
        
        // Living Legend (ultimate achievement)
        let badges_1000 = get_badges_for_sessions(1000);
        assert!(badges_1000.contains(&"⛩️👑 Living Legend".to_string()));
        assert_eq!(badges_1000.len(), 21);
    }

    #[test]
    fn test_legendary_tier() {
        let badges_100 = get_badges_for_sessions(100);
        assert!(badges_100.contains(&"💯 Century Samurai".to_string()));
        
        let badges_200 = get_badges_for_sessions(200);
        assert!(badges_200.contains(&"🌸⚔️ Twin Blades".to_string()));
        
        let badges_365 = get_badges_for_sessions(365);
        assert!(badges_365.contains(&"🏯 Daimyo".to_string()));
        
        let badges_500 = get_badges_for_sessions(500);
        assert!(badges_500.contains(&"🔮 Mystic Warrior".to_string()));
        
        let badges_1000 = get_badges_for_sessions(1000);
        assert!(badges_1000.contains(&"⛩️👑 Living Legend".to_string()));
    }

    #[test]
    fn test_habit_content_structure() {
        let content = HabitContent {
            name: "🗡️ Habit Tracker".to_string(),
            description: "Path to mastery".to_string(),
            owner: "user123".to_string(),
            habit_name: "Morning Meditation".to_string(),
            total_sessions: 7,
            created_at: Some(1000000),
            last_updated: Some(1000000),
            badges: get_badges_for_sessions(7),
        };

        // Verify structure
        assert_eq!(content.total_sessions, 7);
        assert_eq!(content.habit_name, "Morning Meditation");
        assert_eq!(content.owner, "user123");
        
        // Should have 3 badges at session 7
        assert_eq!(content.badges.len(), 3);
        assert!(content.badges.contains(&"🌸 First Blood".to_string()));
        assert!(content.badges.contains(&"⚔️ Three Cuts".to_string()));
        assert!(content.badges.contains(&"🔥 Week Warrior".to_string()));
        
        assert!(content.created_at.is_some());
        assert!(content.last_updated.is_some());
    }

    #[test]
    fn test_min_update_interval_constant() {
        // Verify the constant is set for testing (5 seconds)
        // Production would use 86400 (24 hours)
        assert_eq!(MIN_UPDATE_INTERVAL_SECS, 5);
        
        println!("✓ MIN_UPDATE_INTERVAL_SECS = {} seconds (testing mode)", MIN_UPDATE_INTERVAL_SECS);
    }

    #[test]
    fn test_rejects_time_violation() {
        // TEST: Should REJECT updates that are too soon
        
        let base_time = 1000000i64;
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(base_time - 10000),
            last_updated: Some(base_time),
            badges: get_badges_for_sessions(5),
        };

        let output_too_soon = HabitContent {
            total_sessions: 6,
            last_updated: Some(base_time + 3), // Only 3 seconds - TOO SOON! (MIN is 5)
            badges: get_badges_for_sessions(6),
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output_too_soon);
        
        assert!(!result, "Should REJECT update that's too soon");
        println!("✓ Correctly rejected update after only 3 seconds");
    }

    #[test]
    fn test_accepts_valid_time() {
        // TEST: Should ACCEPT updates after waiting
        
        let base_time = 1000000i64;
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(base_time - 10000),
            last_updated: Some(base_time),
            badges: get_badges_for_sessions(5),
        };

        let output_after_wait = HabitContent {
            total_sessions: 6,
            last_updated: Some(base_time + MIN_UPDATE_INTERVAL_SECS), // Exactly at threshold
            badges: get_badges_for_sessions(6),
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output_after_wait);
        
        assert!(result, "Should ACCEPT update after waiting");
        println!("✓ Correctly accepted update after {} seconds", MIN_UPDATE_INTERVAL_SECS);
    }

    #[test]
    fn test_rejects_owner_change() {
        // TEST: Should REJECT changes to owner
        
        let base_time = 1000000i64;
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "alice123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(base_time - 10000),
            last_updated: Some(base_time),
            badges: get_badges_for_sessions(5),
        };

        let mut output = HabitContent {
            total_sessions: 6,
            last_updated: Some(base_time + MIN_UPDATE_INTERVAL_SECS),
            badges: get_badges_for_sessions(6),
            ..input.clone()
        };
        output.owner = "hacker456".to_string(); // Try to change owner!

        let result = validate_habit_logic(Some(input), output);
        
        assert!(!result, "Should REJECT owner change");
        println!("✓ Correctly rejected attempt to change owner");
    }

    #[test]
    fn test_rejects_invalid_increment() {
        // TEST: Should REJECT session increments != 1
        
        let base_time = 1000000i64;
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(base_time - 10000),
            last_updated: Some(base_time),
            badges: get_badges_for_sessions(5),
        };

        // Try to jump by 2
        let output_skip = HabitContent {
            total_sessions: 7, // Jumped from 5 to 7!
            last_updated: Some(base_time + MIN_UPDATE_INTERVAL_SECS),
            badges: get_badges_for_sessions(7),
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output_skip);
        
        assert!(!result, "Should REJECT increment by 2");
        println!("✓ Correctly rejected session jump (5 → 7)");
    }

    #[test]
    fn test_accepts_valid_increment() {
        // TEST: Should ACCEPT valid increment by 1
        
        let base_time = 1000000i64;
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(base_time - 10000),
            last_updated: Some(base_time),
            badges: get_badges_for_sessions(5),
        };

        let output = HabitContent {
            total_sessions: 6, // Valid +1
            last_updated: Some(base_time + MIN_UPDATE_INTERVAL_SECS),
            badges: get_badges_for_sessions(6),
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output);
        
        assert!(result, "Should ACCEPT valid increment by 1");
        println!("✓ Correctly accepted valid increment (5 → 6)");
    }

    #[test]
    fn test_rejects_wrong_badges() {
        // TEST: Should REJECT incorrect badges
        
        let base_time = 1000000i64;
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(base_time - 10000),
            last_updated: Some(base_time),
            badges: get_badges_for_sessions(5),
        };

        let output = HabitContent {
            total_sessions: 6,
            last_updated: Some(base_time + MIN_UPDATE_INTERVAL_SECS),
            badges: vec!["Wrong Badge".to_string()], // WRONG BADGES!
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output);
        
        assert!(!result, "Should REJECT wrong badges");
        println!("✓ Correctly rejected incorrect badges");
    }

    #[test]
    fn test_accepts_nft_creation() {
        // TEST: Should ACCEPT NFT creation (no input)
        
        let output = HabitContent {
            name: "New Habit".to_string(),
            description: "Brand new".to_string(),
            owner: "newuser123".to_string(),
            habit_name: "Exercise".to_string(),
            total_sessions: 0,
            created_at: Some(1000000),
            last_updated: None,
            badges: vec![],
        };

        // No input - this is creation
        let result = validate_habit_logic(None, output);
        
        assert!(result, "Should ACCEPT NFT creation");
        println!("✓ Correctly accepted NFT creation");
    }

    #[test]
    fn test_accepts_first_update_no_time_check() {
        // TEST: First update (no last_updated in input) should pass without time check
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 0,
            created_at: Some(1000000),
            last_updated: None, // No previous timestamp
            badges: vec![],
        };

        let output = HabitContent {
            total_sessions: 1,
            last_updated: Some(1000001), // Any time is fine for first update
            badges: get_badges_for_sessions(1),
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output);
        
        assert!(result, "Should ACCEPT first update without time restriction");
        println!("✓ Correctly accepted first update (no time check when last_updated is None)");
    }

    #[test]
    fn test_rejects_session_decrement() {
        // TEST: Should REJECT session decrements
        
        let base_time = 1000000i64;
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(base_time - 10000),
            last_updated: Some(base_time),
            badges: get_badges_for_sessions(5),
        };

        let output_decrement = HabitContent {
            total_sessions: 4, // Going backwards!
            last_updated: Some(base_time + MIN_UPDATE_INTERVAL_SECS),
            badges: get_badges_for_sessions(4),
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output_decrement);
        
        assert!(!result, "Should REJECT session decrement");
        println!("✓ Correctly rejected session decrement (5 → 4)");
    }

    #[test]
    fn test_rejects_no_change() {
        // TEST: Should REJECT when sessions don't change
        
        let base_time = 1000000i64;
        
        let input = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(base_time - 10000),
            last_updated: Some(base_time),
            badges: get_badges_for_sessions(5),
        };

        let output_no_change = HabitContent {
            total_sessions: 5, // Same as input
            last_updated: Some(base_time + MIN_UPDATE_INTERVAL_SECS),
            badges: get_badges_for_sessions(5),
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output_no_change);
        
        assert!(!result, "Should REJECT when sessions don't increment");
        println!("✓ Correctly rejected no change in sessions");
    }
}