pub mod config;
pub mod dimensions;
pub mod history;
pub mod scorer;
pub mod types;

pub use scorer::score_stock_pick;
pub use types::{DimensionScore, ScoreWeights, StockScore, score_label};
