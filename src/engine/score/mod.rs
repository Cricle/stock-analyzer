pub mod config;
pub mod dimensions;
pub mod history;
pub mod scorer;
pub mod score_types;

pub use scorer::score_stock_pick;
pub use score_types::{DimensionScore, ScoreWeights, StockScore, score_label};
