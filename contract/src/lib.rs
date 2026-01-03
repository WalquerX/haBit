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

// Badge system combining Robin Sharma's habit formation + Samurai principles
fn get_badges_for_sessions(sessions: u64) -> Vec<String> {
    let mut badges = Vec::new();

    if sessions >= 1 {
        badges.push("First Strike".to_string()); // First step on the path
    }
    if sessions >= 7 {
        badges.push("Week Warrior".to_string()); // 7 days of discipline
    }
    if sessions >= 21 {
        badges.push("Path Beginner".to_string()); // 21-day habit initiation (Robin Sharma)
    }
    if sessions >= 30 {
        badges.push("Moon Master".to_string()); // 30 days - lunar cycle
    }
    if sessions >= 66 {
        badges.push("Habit Forged".to_string()); // 66 days - habit formation (Robin Sharma)
    }
    if sessions >= 90 {
        badges.push("Discipline Disciple".to_string()); // 90 days of mastery
    }
    if sessions >= 100 {
        badges.push("Century Samurai".to_string()); // 100 days milestone
    }
    if sessions >= 180 {
        badges.push("Half-Year Hero".to_string()); // 6 months
    }
    if sessions >= 365 {
        badges.push("Year of the Way".to_string()); // Full year - Bushido path
    }

    badges
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_badge_progression() {
        // Test badge milestones
        assert_eq!(get_badges_for_sessions(0), Vec::<String>::new());
        assert_eq!(get_badges_for_sessions(1), vec!["First Strike"]);
        assert_eq!(
            get_badges_for_sessions(7),
            vec!["First Strike", "Week Warrior"]
        );
        assert_eq!(
            get_badges_for_sessions(21),
            vec!["First Strike", "Week Warrior", "Path Beginner"]
        );
        assert_eq!(
            get_badges_for_sessions(30),
            vec!["First Strike", "Week Warrior", "Path Beginner", "Moon Master"]
        );
        assert_eq!(
            get_badges_for_sessions(66),
            vec![
                "First Strike",
                "Week Warrior",
                "Path Beginner",
                "Moon Master",
                "Habit Forged"
            ]
        );
        assert_eq!(
            get_badges_for_sessions(90),
            vec![
                "First Strike",
                "Week Warrior",
                "Path Beginner",
                "Moon Master",
                "Habit Forged",
                "Discipline Disciple"
            ]
        );
        assert_eq!(
            get_badges_for_sessions(365),
            vec![
                "First Strike",
                "Week Warrior",
                "Path Beginner",
                "Moon Master",
                "Habit Forged",
                "Discipline Disciple",
                "Century Samurai",
                "Half-Year Hero",
                "Year of the Way"
            ]
        );
    }

    #[test]
    fn test_habit_content_structure() {
        let content = HabitContent {
            name: "Test Habit".to_string(),
            description: "Test".to_string(),
            owner: "user123".to_string(),
            habit_name: "Meditation".to_string(),
            total_sessions: 5,
            created_at: Some(1000000),
            last_updated: Some(1000000),
            badges: get_badges_for_sessions(5),
        };

        // Verify structure
        assert_eq!(content.total_sessions, 5);
        assert_eq!(content.habit_name, "Meditation");
        assert_eq!(content.owner, "user123");
        assert_eq!(content.badges, vec!["First Strike"]);
        assert!(content.created_at.is_some());
        assert!(content.last_updated.is_some());
    }

    #[test]
    fn test_min_update_interval_constant() {
        // Verify the constant is set correctly for testing
        assert!(MIN_UPDATE_INTERVAL_SECS >= 10, "Update interval too short for testing");
        assert!(MIN_UPDATE_INTERVAL_SECS <= 300, "Update interval too long for quick testing");
        
        println!("✓ MIN_UPDATE_INTERVAL_SECS = {} seconds", MIN_UPDATE_INTERVAL_SECS);
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
            last_updated: Some(base_time + 30), // Only 30 seconds - TOO SOON!
            badges: get_badges_for_sessions(6),
            ..input.clone()
        };

        let result = validate_habit_logic(Some(input), output_too_soon);
        
        assert!(!result, "Should REJECT update that's too soon");
        println!("✓ Correctly rejected update after only 30 seconds");
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