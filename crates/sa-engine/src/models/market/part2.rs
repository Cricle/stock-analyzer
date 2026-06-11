
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillboardSeatItem {
    pub trade_date: String,
    pub symbol: String,
    pub department_name: String,
    pub buy_amount: Option<f64>,
    pub sell_amount: Option<f64>,
    pub net_amount: Option<f64>,
    pub explanation: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BillboardSeatsResponse {
    pub symbol: String,
    pub market: String,
    pub source: String,
    pub status: String,
    pub side: String,
    pub items: Vec<BillboardSeatItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
