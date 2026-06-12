use serde::Deserialize;

#[derive(Debug, Clone)]
pub(crate) struct AkshareIndividualInfo {
    pub(crate) stock_name: Option<String>,
    pub(crate) total_share: Option<i64>,
    pub(crate) market_cap: Option<f64>,
    pub(crate) industry: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EastmoneyMainFinanceIndicatorItem {
    #[serde(rename = "REPORT_DATE")]
    pub(crate) report_date: Option<String>,
    #[serde(rename = "STD_REPORT_DATE")]
    pub(crate) std_report_date: Option<String>,
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
pub(crate) struct ProfitSheetWire {
    pub(crate) notice_date: Option<String>,
    pub(crate) total_revenue: Option<f64>,
    pub(crate) net_profit: Option<f64>,
    pub(crate) net_profit_deducted: Option<f64>,
}
