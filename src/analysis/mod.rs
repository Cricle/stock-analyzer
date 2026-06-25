use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::task::TaskStatus;

include!("types.rs");
include!("derived.rs");
include!("scenario_types.rs");
include!("report_types.rs");
include!("report_logic/ic_report.rs");
include!("report_logic/core.rs");
include!("report_logic/decision_view.rs");
include!("report_logic/calibration.rs");
include!("report_logic/chart.rs");
include!("report_logic/probability.rs");
include!("report_logic/technical_indicators.rs");
include!("report_logic/news_insights.rs");
include!("report_logic/risk_controls.rs");
include!("report_logic/setup_quality.rs");
include!("report_logic/references.rs");
include!("report_logic/diagnostics.rs");
include!("report_logic/setup_tags.rs");
include!("report_logic/trader_plan.rs");
include!("report_logic/catalyst_review.rs");
