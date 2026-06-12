use super::types::ProviderResult;
use crate::data::{
    AnalystDetail, AnalystRank, BalanceSheet, CandlePoint, CashFlowSheet, CommentDesireIndex,
    CommentFocusIndex, CommentHistScore, CommentOrgParticipation, DividendInfo, DzjyHygtj,
    DzjyHyyybtj, DzjyMrtj, DzjyYybph, EarningsForecast as AkEarningsForecast,
    EarningsQuickReport, EarningsReport, EsgRating, FundFlowEntry, FundamentalsSnapshot,
    GdfxHoldingAnalyse, GdfxHoldingChange, GdfxHoldingDetail, GdfxHoldingStatistic,
    GdfxTeamwork, GdfxTop10, Gdhs, GdhsDetail, Ggcg, GpzyDistributeEntry, GpzyIndustry,
    GpzyPledgeDetail, GpzyPledgeRatio, GpzyPledgeRatioDetail, GpzyProfile, HotStockXq,
    IndustryCategory, JgdyDetail, JgdyTj, LhbDetail, LhbHyyyb, LhbJgmmtj, LhbJgstatistic,
    LhbStockDetail, LhbStockDetailDate, LhbStockStatistic, LhbTraderStatistic, LhbYybDetail,
    LhbYybph, MainFundFlow, MarginAccountInfo, MarginRatioPa, MarginSseDetail, MarginSseSummary,
    MarginSzseDetail, MarginSzseSummary, MarketDataClient, NewsItem, PankouChange, ProfitSheet,
    QuoteSnapshot, SectorFundFlowRank, StockComment, StockSearchResult, ZtPool, ZtPoolDtgc,
    ZtPoolPrevious, ZtPoolStrong, ZtPoolSubNew, ZtPoolZbgc,
    akshare_conv,
};

pub(crate) async fn search_stocks(
    client: &MarketDataClient,
    query: &str,
    market: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<StockSearchResult>> {
    client.ak.a_share_search(query, market, limit).await.map_err(Into::into)
}

pub(crate) async fn fetch_quote(
    client: &MarketDataClient,
    symbol: &str,
    _ts_code: &str,
) -> anyhow::Result<ProviderResult<QuoteSnapshot>> {
    let ak_quote = client.ak.a_share_quote(symbol).await?;
    Ok((akshare_conv::quote_from_akshare(ak_quote), "akshare".to_string()))
}

pub(crate) async fn fetch_fundamentals(
    client: &MarketDataClient,
    symbol: &str,
    ts_code: &str,
) -> anyhow::Result<FundamentalsSnapshot> {
    client.fetch_a_share_fundamentals(symbol, ts_code).await
}

pub(crate) async fn fetch_insider_transactions(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<NewsItem>> {
    let items = client.ak.stock_ggcg_em(symbol).await?;
    Ok(items
        .into_iter()
        .map(|item| {
            let direction = if item.direction.contains("增") || item.direction.to_uppercase() == "IN" {
                "增持"
            } else {
                "减持"
            };
            NewsItem {
                published_at: item.notice_date,
                title: format!("{} {} {}", item.holder_name, direction, item.name),
                summary: format!(
                    "变动{}股, 占总股本{:.4}%, 占流通股{:.4}%, 持股{}股",
                    item.change_amount, item.change_total_ratio, item.change_circulating_ratio, item.holding_count
                ),
                source: "Eastmoney 高管持股".to_string(),
                url: None,
            }
        })
        .collect())
}

pub(crate) async fn fetch_candles(
    client: &MarketDataClient,
    symbol: &str,
    adjust: &str,
    limit: usize,
) -> anyhow::Result<ProviderResult<Vec<CandlePoint>>> {
    let ak_candles = client.ak.a_share_candles(symbol, adjust, limit).await?;
    Ok((ak_candles.into_iter().map(akshare_conv::candle_from_akshare).collect(), "akshare".to_string()))
}

pub(crate) async fn fetch_return_since(
    client: &MarketDataClient,
    symbol: &str,
    start_date: &str,
    holding_days: usize,
) -> anyhow::Result<Option<f64>> {
    client
        .fetch_a_share_return_since(symbol, start_date, holding_days)
        .await
}

// ---------------------------------------------------------------------------
// Fund flow (资金流向)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_fund_flow_individual(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<FundFlowEntry>> {
    client.ak.stock_fund_flow_individual(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_fund_flow_concept(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<SectorFundFlowRank>> {
    client.ak.stock_fund_flow_concept(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_fund_flow_industry(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<SectorFundFlowRank>> {
    client.ak.stock_fund_flow_industry(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_main_fund_flow(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<MainFundFlow>> {
    client.ak.stock_main_fund_flow(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Billboard / Dragon Tiger List (龙虎榜)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_billboard_detail(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<LhbDetail>> {
    client.ak.stock_lhb_detail_em(start_date, end_date).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_stock_statistic(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<LhbStockStatistic>> {
    client.ak.stock_lhb_stock_statistic_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_jgmmtj(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<LhbJgmmtj>> {
    client.ak.stock_lhb_jgmmtj_em(start_date, end_date).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_jgstatistic(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<LhbJgstatistic>> {
    client.ak.stock_lhb_jgstatistic_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_hyyyb(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<LhbHyyyb>> {
    client.ak.stock_lhb_hyyyb_em(start_date, end_date).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_yybph(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<LhbYybph>> {
    client.ak.stock_lhb_yybph_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_trader_statistic(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<LhbTraderStatistic>> {
    client.ak.stock_lhb_traderstatistic_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_stock_detail_date(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<LhbStockDetailDate>> {
    client.ak.stock_lhb_stock_detail_date_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_stock_detail(
    client: &MarketDataClient,
    symbol: &str,
    date: &str,
    flag: &str,
) -> anyhow::Result<Vec<LhbStockDetail>> {
    client.ak.stock_lhb_stock_detail_em(symbol, date, flag).await.map_err(Into::into)
}

pub(crate) async fn fetch_billboard_yyb_detail(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<LhbYybDetail>> {
    client.ak.stock_lhb_yyb_detail_em(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Margin Trading (融资融券)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_margin_account_info(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<MarginAccountInfo>> {
    client.ak.stock_margin_account_info_em(start_date, end_date).await.map_err(Into::into)
}

pub(crate) async fn fetch_margin_sse_detail(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<MarginSseDetail>> {
    client.ak.stock_margin_detail_sse(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_margin_szse_detail(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<MarginSzseDetail>> {
    client.ak.stock_margin_detail_szse(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_margin_ratio_pa(
    client: &MarketDataClient,
    symbol: &str,
    date: &str,
) -> anyhow::Result<Vec<MarginRatioPa>> {
    client.ak.stock_margin_ratio_pa(symbol, date).await.map_err(Into::into)
}

pub(crate) async fn fetch_margin_sse_summary(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<MarginSseSummary>> {
    client.ak.stock_margin_sse(start_date, end_date).await.map_err(Into::into)
}

pub(crate) async fn fetch_margin_szse_summary(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<MarginSzseSummary>> {
    client.ak.stock_margin_szse(date).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Limit-Up/Down Pools (涨停/跌停股池)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_zt_pool(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<ZtPool>> {
    client.ak.stock_zt_pool_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_zt_pool_dtgc(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<ZtPoolDtgc>> {
    client.ak.stock_zt_pool_dtgc_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_zt_pool_previous(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<ZtPoolPrevious>> {
    client.ak.stock_zt_pool_previous_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_zt_pool_strong(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<ZtPoolStrong>> {
    client.ak.stock_zt_pool_strong_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_zt_pool_sub_new(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<ZtPoolSubNew>> {
    client.ak.stock_zt_pool_sub_new_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_zt_pool_zbgc(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<ZtPoolZbgc>> {
    client.ak.stock_zt_pool_zbgc_em(date).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Earnings (业绩)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_earnings_forecast(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<AkEarningsForecast>> {
    client.ak.stock_yjyg_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_earnings_quick_report(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<EarningsQuickReport>> {
    client.ak.stock_yjkb_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_earnings_report(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<EarningsReport>> {
    client.ak.stock_yjbb_em(date).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Analyst (分析师)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_analyst_rank(
    client: &MarketDataClient,
    year: &str,
) -> anyhow::Result<Vec<AnalystRank>> {
    client.ak.stock_analyst_rank_em(year).await.map_err(Into::into)
}

pub(crate) async fn fetch_analyst_detail(
    client: &MarketDataClient,
    analyst_id: &str,
    indicator: &str,
) -> anyhow::Result<Vec<AnalystDetail>> {
    client.ak.stock_analyst_detail_em(analyst_id, indicator).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Shareholder Analysis (股东分析)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_gdfx_free_holding_statistics(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<GdfxHoldingStatistic>> {
    client.ak.stock_gdfx_free_holding_statistics_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_holding_statistics(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<GdfxHoldingStatistic>> {
    client.ak.stock_gdfx_holding_statistics_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_free_holding_change(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<GdfxHoldingChange>> {
    client.ak.stock_gdfx_free_holding_change_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_holding_change(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<GdfxHoldingChange>> {
    client.ak.stock_gdfx_holding_change_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_free_top10(
    client: &MarketDataClient,
    symbol: &str,
    date: &str,
) -> anyhow::Result<Vec<GdfxTop10>> {
    client.ak.stock_gdfx_free_top_10_em(symbol, date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_top10(
    client: &MarketDataClient,
    symbol: &str,
    date: &str,
) -> anyhow::Result<Vec<GdfxTop10>> {
    client.ak.stock_gdfx_top_10_em(symbol, date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_free_holding_detail(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<GdfxHoldingDetail>> {
    client.ak.stock_gdfx_free_holding_detail_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_holding_detail(
    client: &MarketDataClient,
    date: &str,
    indicator: &str,
    symbol: &str,
) -> anyhow::Result<Vec<GdfxHoldingDetail>> {
    client.ak.stock_gdfx_holding_detail_em(date, indicator, symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_free_holding_analyse(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<GdfxHoldingAnalyse>> {
    client.ak.stock_gdfx_free_holding_analyse_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_holding_analyse(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<GdfxHoldingAnalyse>> {
    client.ak.stock_gdfx_holding_analyse_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_free_teamwork(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<GdfxTeamwork>> {
    client.ak.stock_gdfx_free_holding_teamwork_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_gdfx_teamwork(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<GdfxTeamwork>> {
    client.ak.stock_gdfx_holding_teamwork_em(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Block Trades (大宗交易)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_block_trade_daily(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<DzjyMrtj>> {
    client.ak.stock_dzjy_mrtj(start_date, end_date).await.map_err(Into::into)
}

pub(crate) async fn fetch_block_trade_industry(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<DzjyHygtj>> {
    client.ak.stock_dzjy_hygtj(start_date, end_date).await.map_err(Into::into)
}

pub(crate) async fn fetch_block_trade_industry_daily(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<DzjyHyyybtj>> {
    client.ak.stock_dzjy_hyyybtj(start_date, end_date).await.map_err(Into::into)
}

pub(crate) async fn fetch_block_trade_seat_ranking(
    client: &MarketDataClient,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<DzjyYybph>> {
    client.ak.stock_dzjy_yybph(start_date, end_date).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Hot Stocks (雪球热度)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_hot_follow_xq(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<HotStockXq>> {
    client.ak.stock_hot_follow_xq(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_hot_tweet_xq(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<HotStockXq>> {
    client.ak.stock_hot_tweet_xq(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_hot_deal_xq(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<HotStockXq>> {
    client.ak.stock_hot_deal_xq(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Order Book Changes (盘口异动)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_pankou_changes(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<PankouChange>> {
    client.ak.stock_changes_em(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Dividends (分红送配)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_dividends(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<DividendInfo>> {
    client.ak.stock_fhps_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_dividend_detail(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<DividendInfo>> {
    client.ak.stock_fhps_detail_em(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Pledge Data (股权质押)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_pledge_profile(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<GpzyProfile>> {
    client.ak.stock_gpzy_profile_em().await.map_err(Into::into)
}

pub(crate) async fn fetch_pledge_ratio(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<GpzyPledgeRatio>> {
    client.ak.stock_gpzy_pledge_ratio_em().await.map_err(Into::into)
}

pub(crate) async fn fetch_pledge_detail(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<GpzyPledgeDetail>> {
    client.ak.stock_gpzy_pledge_detail_em().await.map_err(Into::into)
}

pub(crate) async fn fetch_pledge_ratio_detail(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<GpzyPledgeRatioDetail>> {
    client.ak.stock_gpzy_individual_pledge_ratio_detail_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_pledge_distribute_bank(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<GpzyDistributeEntry>> {
    client.ak.stock_gpzy_distribute_statistics_bank_em().await.map_err(Into::into)
}

pub(crate) async fn fetch_pledge_distribute_company(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<GpzyDistributeEntry>> {
    client.ak.stock_gpzy_distribute_statistics_company_em().await.map_err(Into::into)
}

pub(crate) async fn fetch_pledge_industry(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<GpzyIndustry>> {
    client.ak.stock_gpzy_industry_data_em().await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Institutional Research (机构调研)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_institutional_research(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<JgdyTj>> {
    client.ak.stock_jgdy_tj_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_institutional_research_detail(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<JgdyDetail>> {
    client.ak.stock_jgdy_detail_em(date).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// ESG Ratings
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_esg_msci(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<EsgRating>> {
    client.ak.stock_esg_msci_sina().await.map_err(Into::into)
}

pub(crate) async fn fetch_esg_rft(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<EsgRating>> {
    client.ak.stock_esg_rft_sina().await.map_err(Into::into)
}

pub(crate) async fn fetch_esg_zd(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<EsgRating>> {
    client.ak.stock_esg_zd_sina().await.map_err(Into::into)
}

pub(crate) async fn fetch_esg_hz(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<EsgRating>> {
    client.ak.stock_esg_hz_sina().await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Financial Reports (三大报表)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_balance_sheet(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<BalanceSheet>> {
    client.ak.stock_zcfz_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_profit_sheet(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<ProfitSheet>> {
    client.ak.stock_lrb_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_cash_flow_sheet(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<CashFlowSheet>> {
    client.ak.stock_xjll_em(date).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Stock Comments (千股千评)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_stock_comments(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<StockComment>> {
    client.ak.stock_comment_em().await.map_err(Into::into)
}

pub(crate) async fn fetch_comment_org_participation(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<CommentOrgParticipation>> {
    client.ak.stock_comment_detail_zlkp_jgcyd_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_comment_hist_score(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<CommentHistScore>> {
    client.ak.stock_comment_detail_zhpj_lspf_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_comment_focus_index(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<CommentFocusIndex>> {
    client.ak.stock_comment_detail_scrd_focus_em(symbol).await.map_err(Into::into)
}

pub(crate) async fn fetch_comment_desire_index(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<CommentDesireIndex>> {
    client.ak.stock_comment_detail_scrd_desire_em(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Shareholder Changes (高管持股变动)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_executive_shareholding(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<Ggcg>> {
    client.ak.stock_ggcg_em(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Shareholder Count (股东户数)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_shareholder_count(
    client: &MarketDataClient,
    date: &str,
) -> anyhow::Result<Vec<Gdhs>> {
    client.ak.stock_gdhs_em(date).await.map_err(Into::into)
}

pub(crate) async fn fetch_shareholder_count_detail(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<GdhsDetail>> {
    client.ak.stock_gdhs_detail_em(symbol).await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Industry Classification (行业分类)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_industry_category(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<IndustryCategory>> {
    client.ak.stock_industry_category_cninfo().await.map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Akshare News Sources (全球资讯)
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_global_news_cls(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<crate::types::NewsItem>> {
    let entries = client.ak.stock_info_global_cls().await?;
    Ok(entries.into_iter().map(|e| crate::types::NewsItem {
        published_at: e.time,
        title: e.title.clone(),
        summary: e.summary.unwrap_or(e.title),
        source: "CLS 财联社".to_string(),
        url: e.url,
    }).collect())
}

pub(crate) async fn fetch_global_news_ths(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<crate::types::NewsItem>> {
    let entries = client.ak.stock_info_global_ths().await?;
    Ok(entries.into_iter().map(|e| crate::types::NewsItem {
        published_at: e.time,
        title: e.title.clone(),
        summary: e.summary.unwrap_or(e.title),
        source: "THS 同花顺".to_string(),
        url: e.url,
    }).collect())
}

pub(crate) async fn fetch_global_news_sina(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<crate::types::NewsItem>> {
    let entries = client.ak.stock_info_global_sina().await?;
    Ok(entries.into_iter().map(|e| crate::types::NewsItem {
        published_at: e.time,
        title: e.title.clone(),
        summary: e.summary.unwrap_or(e.title),
        source: "Sina 新浪".to_string(),
        url: e.url,
    }).collect())
}

pub(crate) async fn fetch_global_news_futu(
    client: &MarketDataClient,
) -> anyhow::Result<Vec<crate::types::NewsItem>> {
    let entries = client.ak.stock_info_global_futu().await?;
    Ok(entries.into_iter().map(|e| crate::types::NewsItem {
        published_at: e.time,
        title: e.title.clone(),
        summary: e.summary.unwrap_or(e.title),
        source: "Futu 富途".to_string(),
        url: e.url,
    }).collect())
}

