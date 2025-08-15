mod rpc;

use crate::{job::Job, share::Share};
use rpc::{
    request::{LoginParams, KeepAlivedParams, Request, SubmitParams},
    response::{LoginResult, Response, StatusResult, SubscribeResult},
};
use serde::Deserialize;
use std::{
    io::{self, BufReader, BufWriter, BufRead},
    net::TcpStream,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

use rpc::response::{SetDifficultyParams, SetExtranonceParams};

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum MiningNotifyParams {
    Array(Vec<serde_json::Value>),
    Object {
        job_id: String,
        blob_hex: String, 
        seed_hash_hex: String,
    },
}

impl TryFrom<MiningNotifyParams> for Job {
    type Error = Box<dyn std::error::Error>;

    fn try_from(params: MiningNotifyParams) -> Result<Self, Self::Error> {
        match params {
            MiningNotifyParams::Array(arr) => {
                if arr.len() < 3 {
                    return Err("mining.notify array must have at least 3 elements".into());
                }
                
                let job_id = arr[0].as_str()
                    .ok_or("job_id must be a string")?
                    .to_string();
                let blob_hex = arr[1].as_str()
                    .ok_or("blob_hex must be a string")?;
                let seed_hash_hex = arr[2].as_str()
                    .ok_or("seed_hash_hex must be a string")?;
                
                Ok(Job {
                    id: job_id,
                    blob: hex::decode(blob_hex)?,
                    seed: hex::decode(seed_hash_hex)?,
                    target: u32::MAX, 
                })
            },
            MiningNotifyParams::Object { job_id, blob_hex, seed_hash_hex } => {
                Ok(Job {
                    id: job_id,
                    blob: hex::decode(blob_hex)?,
                    seed: hex::decode(seed_hash_hex)?,
                    target: u32::MAX, 
                })
            }
        }
    }
}


#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum PoolMessage {
    MiningNotify(Request<MiningNotifyParams>), // For standard mining.notify messages
    NewJob(Request<Job>), // For job messages with method "job" (e.g. initial job)
    SetDifficulty(Request<SetDifficultyParams>),
    SetExtranonce(Request<SetExtranonceParams>),
    ResponseSubscribe(Response<SubscribeResult>),
    ResponseBool(Response<bool>),
    Response(Response<StatusResult>), // Simplified response handling, based on working example
}

#[derive(Debug)]
pub struct Stratum {
    url: String,
    user: String,
    pass: String,
    login_id: String,
    writer: BufWriter<TcpStream>,
    job_rx: Receiver<Job>,
    reconnect_tx: mpsc::Sender<()>,
    reconnect_rx: Receiver<()>,
}

impl Stratum {
    #[tracing::instrument]
    fn _connect_and_login(
        url: &str,
        user: &str,
        pass: &str,
    ) -> io::Result<(
        String,
        BufWriter<TcpStream>,
        Receiver<Job>,
        mpsc::Sender<()>,
        Receiver<()>,
    )> {
        let stream = TcpStream::connect(url)?;
        stream.set_read_timeout(None)?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut writer = BufWriter::new(stream.try_clone()?);

        let (job_tx, job_rx) = mpsc::channel();
        let (reconnect_tx, reconnect_rx) = mpsc::channel();

        let login_id: String;
        let initial_job: Job;

        tracing::debug!("Sending login.");
        rpc::send(
            &mut writer,
            &Request::new_login(LoginParams {
                login: user.into(),
                pass: pass.into(),
            }),
        )?;
        let response = rpc::recv::<Response<LoginResult>>(&mut reader)?;
        if let Some(result) = response.result {
            let LoginResult { id, job, .. } = result;
            tracing::debug!("Received initial job from pool: {}", job.id);
            login_id = id;
            initial_job = job;
        } else {
            let msg = response.error.unwrap().message;
            tracing::warn!("{}", msg);
            return Err(io::Error::other(msg));
        }

        job_tx.send(initial_job).unwrap();
        let reconnect_tx_clone = reconnect_tx.clone();
        thread::spawn(move || {
            let span = tracing::info_span!("listener");
            let _enter = span.enter();
                loop {
                    let mut line = String::new();
                    let read_result = reader.read_line(&mut line);
                    if read_result.is_err() || line.is_empty() {
                        let e = read_result.err().unwrap_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "EOF while reading line"));
                        tracing::error!("Connection error in listener (read_line): {}", e);
                        reconnect_tx_clone.send(()).unwrap();
                        break;
                    }
                    tracing::debug!("Raw JSON from pool: {}", line.trim());
                    
                    // Attempt to parse the JSON to understand its structure before specific deserialization.
                    // This helps in debugging issues with pool messages that might not conform strictly to expected types.
                    match serde_json::from_str::<serde_json::Value>(&line) {
                        Ok(json_value) => {
                            tracing::debug!("Parsed JSON structure: {:#}", json_value);
                            
                            // Log the method type if present, to aid in understanding message flow.
                            if let Some(method) = json_value.get("method").and_then(|m| m.as_str()) {
                                tracing::info!("Received method call: {}", method);
                                // Specific tracing for known methods can be useful for filtering logs.
                                match method {
                                    "mining.notify" | "job" => {
                                        tracing::debug!("Method '{}' identified, proceeding to specific parsing.", method);
                                    },
                                    _ => {
                                        tracing::debug!("Received unhandled method: {}", method);
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            tracing::error!("Failed to parse JSON into a generic Value: {}", e);
                        }
                    }
                    
                    match serde_json::from_str::<PoolMessage>(&line) {
                        Ok(msg) => match msg {
                            PoolMessage::Response(response) => {
                                if let Some(err) = response.error {
                                    tracing::warn!("{}", err.message);
                                } else if let Some(status_result) = response.result {
                                    match status_result.status.as_str() {
                                        "OK" => {
                                            tracing::info!("Share accepted by pool.");
                                        },
                                        "KEEPALIVED" => tracing::debug!("keepalived"),
                                        _ => tracing::warn!("Unknown status: {}", status_result.status),
                                    }
                                } else {
                                    tracing::warn!("Received response with no error and no result.");
                                }
                            }
                            PoolMessage::ResponseBool(response) => {
                                if let Some(err) = response.error {
                                    tracing::warn!("{}", err.message);
                                } else {
                                    tracing::debug!("Received boolean response: {:?}", response.result);
                                }
                            }
                            PoolMessage::ResponseSubscribe(response) => {
                                if let Some(err) = response.error {
                                    tracing::warn!("{}", err.message);
                                } else {
                                    tracing::debug!("Received subscribe response in listener: {:?}", response.result);
                                }
                            }
                            PoolMessage::NewJob(request) => {
                                tracing::info!(job_id = %request.params.id, "Received new job from pool (method 'job').");
                                if let Err(e) = job_tx.send(request.params) {
                                    tracing::error!("Failed to send job to worker: {}", e);
                                    reconnect_tx_clone.send(()).unwrap();
                                    break;
                                }
                            }
                            PoolMessage::MiningNotify(request) => {
                                tracing::info!("Received new job from pool (method 'mining.notify').");
                                match Job::try_from(request.params) {
                                    Ok(job) => {
                                        let job_id = job.id.clone();
                                        tracing::info!(job_id = %job_id, "Successfully parsed mining.notify job.");
                                        if let Err(e) = job_tx.send(job) {
                                            tracing::error!("Failed to send job to worker: {}", e);
                                            reconnect_tx_clone.send(()).unwrap();
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(error = %e, "Failed to convert mining.notify params to Job.");
                                    }
                                }
                            }
                            PoolMessage::SetDifficulty(request) => {
                                let SetDifficultyParams::Array(params) = request.params;
                                if let Some(difficulty_value) = params[0].as_u64() {
                                    tracing::info!("Received mining.set_difficulty in listener: {}", difficulty_value);
                                } else {
                                    tracing::warn!("Invalid difficulty value in mining.set_difficulty in listener.");
                                }
                            },
                            PoolMessage::SetExtranonce(request) => {
                                let SetExtranonceParams::Array(params) = request.params;
                                let extranonce = params[0].as_str().unwrap_or_default().to_string();
                                let extranonce_size = params[1].as_u64().unwrap_or_default();
                                tracing::info!("Received mining.set_extranonce in listener: extranonce={}, size={}", extranonce, extranonce_size);
                            },
                        },
                        Err(e) => {
                            tracing::error!("Connection error in listener: {}", e);
                            reconnect_tx_clone.send(()).unwrap();
                            break;
                        }
                    }
                }
            });
        Ok((
            login_id,
            writer,
            job_rx,
            reconnect_tx,
            reconnect_rx,
        ))
    }

    #[tracing::instrument]
    pub fn login(url: &str, user: &str, pass: &str) -> io::Result<Self> {
        let (login_id, writer, job_rx, reconnect_tx, reconnect_rx) =
            Self::_connect_and_login(url, user, pass)?;
        Ok(Self {
            url: url.into(),
            user: user.into(),
            pass: pass.into(),
            login_id,
            writer,
            job_rx,
            reconnect_tx,
            reconnect_rx,
        })
    }

    pub fn submit(&mut self, share: Share) -> io::Result<()> {
        tracing::info!("Submitting share for job_id: {}", share.job_id);
        rpc::send(
            &mut self.writer,
            &Request::new_submit_standard(SubmitParams {
                id: self.login_id.clone(),
                job_id: share.job_id,
                nonce: share.nonce,
                result: share.hash,
            }),
        )?;
        tracing::debug!("Share submitted, awaiting new job from pool.");
        Ok(())
    }
    pub fn keep_alive(&mut self) -> io::Result<()> {
        rpc::send(
            &mut self.writer,
            &Request::new_keep_alive(KeepAlivedParams {
                id: self.login_id.clone(),
            }),
        )
    }
    pub fn try_recv_job(&self) -> Result<Job, TryRecvError> {
        self.job_rx.try_recv()
    }

    #[tracing::instrument]
    pub fn reconnect(&mut self) -> io::Result<()> {
        tracing::info!("Attempting to reconnect...");
        let (login_id, writer, job_rx, reconnect_tx, reconnect_rx) =
            Self::_connect_and_login(&self.url, &self.user, &self.pass)?;

        self.login_id = login_id;
        self.writer = writer;
        self.job_rx = job_rx;
        self.reconnect_tx = reconnect_tx;
        self.reconnect_rx = reconnect_rx;

        tracing::info!("Reconnected successfully!");
        Ok(())
    }

    pub fn try_reconnect_signal(&self) -> Result<(), TryRecvError> {
        self.reconnect_rx.try_recv()
    }
}
