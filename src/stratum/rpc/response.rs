use crate::job::Job;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(dead_code)]
#[derive(Deserialize, Debug, Serialize)]
pub struct Error {
    pub code: i32,
    pub message: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Response<R> {
    pub result: Option<R>,
    pub error: Option<Error>,
    pub id: u32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LoginResult {
    pub job: Job,
    pub id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct StatusResult {
    pub status: String,
}

// For "mining.subscribe" method (standard)
// Result is typically an array: [["mining.set_difficulty", "mining.notify"], "extranonce", extranonce2_size]
#[derive(Debug, Deserialize)]
pub struct SubscribeResult {
    #[serde(default)]
    #[allow(dead_code)]
    pub result: Vec<Value>,
}

// For "mining.notify" method (Standard Stratum v1 style - array of params)
// Params is an array: ["JOB_ID", "BLOB_DATA", "SEED_HASH", null, null, null, "TARGET_DIFFICULTY", true]
// For "job" method (Alternative style - object of params)
// Params is an object: {"id": "LOGIN_ID", "job_id": "JOB_ID", "blob": "BLOB_DATA", "target": "TARGET", "seed_hash": "SEED_HASH", ...}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum NotifyParamsNiceHash {
    #[allow(dead_code)]
    Array(Vec<Value>),
    #[allow(dead_code)]
    Object(NotifyParamsObject),
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct NotifyParamsObject {
    pub id: String,
    pub job_id: String,
    pub blob: String,
    pub target: String,
    pub seed_hash: String,
    // next_seed_hash can be empty, so we don't need to parse it if it's not always present or critical
    // pub next_seed_hash: Option<String>,
    // pub algo: Option<String>,
    // pub height: Option<u64>,
}

// For "mining.set_difficulty" method (Server to Miner)
// Params is an array: [DIFFICULTY]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SetDifficultyParams {
    Array(Vec<Value>),
}

// For "mining.set_extranonce" method (Server to Miner)
// Params is an array: ["extranonce", extranonce_size]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SetExtranonceParams {
    Array(Vec<Value>),
}
