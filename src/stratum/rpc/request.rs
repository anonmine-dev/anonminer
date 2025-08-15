use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct Request<P> {
    pub method: String,
    pub params: P,
    #[serde(skip_deserializing)]
    pub id: u32,
}

// For "login" method (non-NiceHash)
#[derive(Debug, Serialize)]
pub struct LoginParams {
    pub login: String,
    pub pass: String,
}

impl Request<LoginParams> {
    pub fn new_login(params: LoginParams) -> Self {
        Self {
            method: "login".into(),
            params,
            id: 1,
        }
    }
}

// For "mining.subscribe" method (standard)
impl Request<Vec<Value>> {
    #[allow(dead_code)]
    pub fn new_subscribe_standard(_user_agent: Option<String>) -> Self {
        // Many pools expect an empty params array for the initial subscribe.
        // The user agent is often handled implicitly or via other means.
        let params = Vec::new();
        Self {
            method: "mining.subscribe".into(),
            params,
            id: 1,
        }
    }


    // For "mining.extranonce.subscribe" method
    #[allow(dead_code)]
    pub fn new_extranonce_subscribe() -> Self {
        Self {
            method: "mining.extranonce.subscribe".into(),
            params: Vec::new(),
            id: 3,
        }
    }

}

// For "submit" method (non-NiceHash)
#[derive(Debug, Serialize)]
pub struct SubmitParams {
    pub id: String,
    pub job_id: String,
    #[serde(with = "hex")]
    pub nonce: Vec<u8>,
    #[serde(with = "hex")]
    pub result: Vec<u8>,
}

impl Request<SubmitParams> {
    pub fn new_submit_standard(params: SubmitParams) -> Self {
        Self {
            method: "submit".into(),
            params,
            id: 1,
        }
    }
}

// For "keepalived" method
#[derive(Debug, Serialize)]
pub struct KeepAlivedParams {
    pub id: String,
}

impl Request<KeepAlivedParams> {
    pub fn new_keep_alive(params: KeepAlivedParams) -> Self {
        Self {
            method: "keepalived".into(),
            params,
            id: 1,
        }
    }
}
