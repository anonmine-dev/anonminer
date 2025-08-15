use std::time::Duration;

#[derive(Clone, Debug)]
pub struct GuiData {
    pub hash_rate: f64,
    pub total_hashes: u64,
    pub elapsed_time: Duration,
    pub shares_found: usize,
    pub is_warming_up: bool,
}
