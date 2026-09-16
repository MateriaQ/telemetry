use rovo::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, PartialEq, Eq)]
pub struct Health {
    pub status: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, PartialEq, Eq)]
pub struct AuthResponse {
    pub user_id: String,
    pub signature: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub error: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PingResponse {
    Ok,
    MissingUserId,
    InvalidUserId,
    InvalidAppId,
    Unauthorized,
    ServiceUnavailable,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct DailyStat {
    pub date: String,
    pub count: u64,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct AppTelemetry {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    pub live: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    pub weekly_history: Vec<DailyStat>,
    pub weekly_max: u64,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct TelemetryOverview {
    pub live: u64,
    pub total: u64,
    pub apps: Vec<AppTelemetry>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum UsersResponse {
    Overview(TelemetryOverview),
    App(AppTelemetry),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct LiveUsersResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub live: u64,
}
