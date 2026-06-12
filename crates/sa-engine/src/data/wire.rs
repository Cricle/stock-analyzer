use std::collections::HashMap;

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub(crate) struct AkshareIndividualInfo {
    pub(crate) stock_name: Option<String>,
    pub(crate) total_share: Option<i64>,
    pub(crate) market_cap: Option<f64>,
    pub(crate) industry: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TushareResponse {
    pub(crate) code: i32,
    pub(crate) msg: Option<String>,
    pub(crate) data: Option<TushareData>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TushareData {
    pub(crate) fields: Vec<String>,
    pub(crate) items: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EastmoneyDatacenterEnvelope<T> {
    pub(crate) result: Option<EastmoneyDatacenterResult<T>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EastmoneyDatacenterResult<T> {
    pub(crate) data: Option<Vec<T>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EastmoneyMainFinanceIndicatorItem {
    #[allow(dead_code)]
    #[serde(rename = "SECUCODE")]
    pub(crate) secucode: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "SECURITY_CODE")]
    pub(crate) security_code: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "SECURITY_NAME_ABBR")]
    pub(crate) security_name_abbr: Option<String>,
    #[serde(rename = "REPORT_DATE")]
    pub(crate) report_date: Option<String>,
    #[serde(rename = "STD_REPORT_DATE")]
    pub(crate) std_report_date: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "REPORT_TYPE")]
    pub(crate) report_type: Option<String>,
    #[serde(rename = "CURRENCY")]
    pub(crate) currency: Option<String>,
    #[serde(rename = "OPERATE_INCOME")]
    pub(crate) operate_income: Option<f64>,
    #[serde(rename = "TOTALOPERATEREVE")]
    pub(crate) total_operate_reve: Option<f64>,
    #[serde(rename = "GROSS_PROFIT")]
    pub(crate) gross_profit: Option<f64>,
    #[serde(rename = "MLR")]
    pub(crate) mlr: Option<f64>,
    #[serde(rename = "HOLDER_PROFIT")]
    pub(crate) holder_profit: Option<f64>,
    #[serde(rename = "PARENTNETPROFIT")]
    pub(crate) parent_net_profit: Option<f64>,
    #[serde(rename = "TOTAL_ASSETS")]
    pub(crate) total_assets: Option<f64>,
    #[serde(rename = "TOTALASSETS")]
    pub(crate) totalassets: Option<f64>,
    #[serde(rename = "TOTAL_LIABILITIES")]
    pub(crate) total_liabilities: Option<f64>,
    #[serde(rename = "TOTLIAB")]
    pub(crate) totliab: Option<f64>,
    #[serde(rename = "TOTAL_PARENT_EQUITY")]
    pub(crate) total_parent_equity: Option<f64>,
    #[serde(rename = "NETCASH_OPERATE")]
    pub(crate) netcash_operate: Option<f64>,
    #[serde(rename = "MGJYXJJE")]
    pub(crate) mgjyxjje: Option<f64>,
    #[serde(rename = "BPS")]
    pub(crate) bps: Option<f64>,
    #[serde(rename = "ZCFZL")]
    pub(crate) zcfzl: Option<f64>,
    #[serde(rename = "CURRENT_LIABILITY")]
    pub(crate) current_liability: Option<f64>,
    #[serde(rename = "CURRENT_LIAB")]
    pub(crate) current_liab: Option<f64>,
    #[serde(rename = "NONCURRENT_LIAB_1YEAR")]
    pub(crate) noncurrent_liab_1year: Option<f64>,
    #[serde(rename = "TOTALNONCLIAB")]
    pub(crate) totalnoncliab: Option<f64>,
    #[serde(rename = "CAPITAL_EXPENDITURE")]
    pub(crate) capital_expenditure: Option<f64>,
    #[serde(rename = "TOTAL_SHARE")]
    pub(crate) total_share: Option<f64>,
    #[allow(dead_code)]
    #[serde(rename = "BASIC_EPS")]
    pub(crate) basic_eps: Option<f64>,
    #[allow(dead_code)]
    #[serde(rename = "DILUTED_EPS")]
    pub(crate) diluted_eps: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EastmoneyBalanceSheetItem {
    #[allow(dead_code)]
    #[serde(rename = "REPORT_DATE")]
    pub(crate) report_date: Option<String>,
    #[serde(rename = "TOTAL_ASSETS")]
    pub(crate) total_assets: Option<f64>,
    #[serde(rename = "TOTAL_LIABILITIES")]
    pub(crate) total_liabilities: Option<f64>,
    #[serde(rename = "TOTAL_EQUITY")]
    pub(crate) total_equity: Option<f64>,
    #[serde(rename = "MONETARYFUNDS")]
    pub(crate) monetary_funds: Option<f64>,
    #[serde(rename = "CURRENT_LIAB")]
    pub(crate) current_liab: Option<f64>,
    #[serde(rename = "TOTALNONCLIAB")]
    pub(crate) totalnoncliab: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EastmoneyCashflowItem {
    #[allow(dead_code)]
    #[serde(rename = "REPORT_DATE")]
    pub(crate) report_date: Option<String>,
    #[serde(rename = "NETCASH_OPERATE")]
    pub(crate) netcash_operate: Option<f64>,
    #[serde(rename = "CONSTRUCT_LONG_ASSET")]
    pub(crate) construct_long_asset: Option<f64>,
    #[serde(rename = "END_CCE")]
    pub(crate) end_cce: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct TushareRow {
    values: HashMap<String, serde_json::Value>,
}
impl TushareRow {
    pub(crate) fn new(fields: &[String], items: Vec<serde_json::Value>) -> Self {
        let values = fields.iter().cloned().zip(items).collect::<HashMap<_, _>>();
        Self { values }
    }

    pub(crate) fn string(&self, key: &str) -> anyhow::Result<String> {
        self.optional_string(key)
            .context(format!("missing string field {}", key))
    }

    pub(crate) fn optional_string(&self, key: &str) -> Option<String> {
        self.values.get(key).and_then(|value| match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            serde_json::Value::Null => None,
            other => Some(other.to_string()),
        })
    }

    pub(crate) fn f64(&self, key: &str) -> anyhow::Result<f64> {
        self.optional_f64(key)
            .context(format!("missing numeric field {}", key))
    }

    pub(crate) fn optional_f64(&self, key: &str) -> Option<f64> {
        self.values.get(key).and_then(|value| match value {
            serde_json::Value::Number(value) => value.as_f64(),
            serde_json::Value::String(value) => value.parse().ok(),
            _ => None,
        })
    }
}
