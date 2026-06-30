use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct HandHistoryRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub site: String,
    pub original_filename: Option<String>,
    #[serde(skip_serializing)]
    pub raw_text: String,
    pub hand_count: i32,
    pub status: String,
    pub error: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub parsed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ParsedHandRecord {
    pub id: Uuid,
    pub hand_history_id: Uuid,
    pub user_id: Uuid,
    pub site: String,
    pub site_hand_id: Option<String>,
    pub game_type: String,
    pub table_size: i32,
    pub hero_position: Option<String>,
    pub big_blind: Option<f64>,
    pub played_at: Option<DateTime<Utc>>,
    pub board: Vec<String>,
    pub hero_cards: Vec<String>,
    pub actions: serde_json::Value,
    pub result_bb: f64,
    pub went_to_showdown: bool,
    pub won_hand: bool,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PlayerStatsRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub game_type: String,
    pub sample_size: i32,
    pub vpip: Option<f64>,
    pub pfr: Option<f64>,
    pub three_bet: Option<f64>,
    pub fold_to_three_bet: Option<f64>,
    pub cbet_flop: Option<f64>,
    pub fold_to_cbet_flop: Option<f64>,
    pub cbet_turn: Option<f64>,
    pub wtsd: Option<f64>,
    pub won_at_showdown: Option<f64>,
    pub aggression_factor: Option<f64>,
    pub net_bb_per_100: Option<f64>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SolvedSpotRecord {
    pub cache_key: String,
    pub game_type: String,
    pub pot_type: String,
    pub stack_bb: i32,
    pub hero_position: String,
    pub villain_position: String,
    pub board: Vec<String>,
    pub response: serde_json::Value,
    pub iterations: i32,
    pub solved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SolveJobRecord {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub request: serde_json::Value,
    pub status: String,
    pub progress: f32,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RangeRecord {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub game_type: String,
    pub range_string: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct DrillRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub game_type: String,
    pub category: String,
    pub spot_key: Option<String>,
    pub spot_snapshot: serde_json::Value,
    pub dealt_hand: Vec<String>,
    pub correct_strategy: serde_json::Value,
    pub correct_ev_bb: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct DrillAttemptRecord {
    pub id: Uuid,
    pub drill_id: Uuid,
    pub user_id: Uuid,
    pub chosen_action: String,
    pub ev_loss_bb: f64,
    pub is_correct: bool,
    pub explanation: String,
    pub answered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WeaknessProfileRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub game_type: String,
    pub category: String,
    pub attempts: i32,
    pub correct: i32,
    pub avg_ev_loss_bb: f64,
    pub last_seen_at: DateTime<Utc>,
}
