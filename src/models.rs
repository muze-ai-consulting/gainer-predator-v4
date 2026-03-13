use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum Side {
    Long,
    Short,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Signal {
    pub msg_id: i32,
    pub symbol: String,
    pub side: Side,
    pub leverage: Option<u32>,
    pub entry: Option<f64>,
    pub sl: Option<f64>,
    pub tp: Option<f64>,
    #[serde(skip, default = "std::time::Instant::now")]
    pub received_at: std::time::Instant,
}
