use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCountResponse {
    pub count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfileResponse {
    pub id: String,
    pub discord_id: Option<String>,
    pub email: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardOverviewResponse {
    pub user: UserProfile,
    pub stats: DashboardStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub email: String,
    pub discord_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_jobs: u64,
    pub jobs_this_month: u64,
    /// Seconds of media transcribed, all time.
    pub media_seconds_total: i64,
    pub media_seconds_this_month: i64,
    pub words_transcribed: i64,
    pub active_api_keys: u32,
    pub subscription_usage: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApplyReferralCodeRequest {
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApplyReferralCodeResponse {
    pub success: bool,
    pub message: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountDeletionCheckResponse {
    pub can_delete: bool,
    pub reason: Option<String>,
    pub subscription_status: Option<String>,
}

/// One item in the onboarding checklist. `complete` is server-derived from
/// the user's actual state; `dismissed` is a user preference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnboardingTodo {
    pub key: String,
    pub complete: bool,
    pub dismissed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnboardingResponse {
    pub todos: Vec<OnboardingTodo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DismissOnboardingTodoRequest {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub date: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicStatsResponse {
    pub user_count: i64,
    pub user_growth: Vec<TimeSeriesPoint>,
    /// Jobs finished per day.
    pub transcription_jobs: Vec<TimeSeriesPoint>,
    pub transcription_total: i64,
    /// Hours of media transcribed, all time.
    pub hours_transcribed: i64,
    pub hours_growth: Vec<TimeSeriesPoint>,
    pub words_transcribed: i64,
    pub languages_seen: i64,
}
