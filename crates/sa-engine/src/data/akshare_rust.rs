use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

use super::{
    CandlePoint, CapitalFlowPoint, FundamentalsSnapshot, MarketDataClient, MarketKind, NewsItem,
    QuoteSnapshot,
};
use akshare::types::StockSearchResult;

type ProviderResult<T> = (T, String);

// ---------------------------------------------------------------------------
// Conversion utilities from akshare f64-based types to our Decimal-based types.
// ---------------------------------------------------------------------------

fn f64_to_dec(v: f64) -> Decimal {
    Decimal::from_f64(v).unwrap_or_default()
}

pub(crate) fn quote_from_akshare(q: akshare::QuoteSnapshot) -> QuoteSnapshot {
    QuoteSnapshot {
        symbol: q.symbol,
        date: q.date,
        open: f64_to_dec(q.open),
        high: f64_to_dec(q.high),
        low: f64_to_dec(q.low),
        close: f64_to_dec(q.close),
        volume: q.volume,
    }
}

pub(crate) fn candle_from_akshare(c: akshare::CandlePoint) -> CandlePoint {
    CandlePoint {
        trade_date: c.trade_date,
        open: f64_to_dec(c.open),
        close: f64_to_dec(c.close),
        high: f64_to_dec(c.high),
        low: f64_to_dec(c.low),
        volume: c.volume,
        amount: f64_to_dec(c.amount),
        amplitude_pct: c.amplitude_pct,
        change_pct: c.change_pct,
        change_amount: f64_to_dec(c.change_amount),
        turnover_pct: c.turnover_pct,
    }
}

pub(crate) fn capital_flow_from_akshare(c: akshare::CapitalFlowPoint) -> CapitalFlowPoint {
    CapitalFlowPoint {
        trade_date: c.trade_date,
        main_net_inflow: f64_to_dec(c.main_net_inflow),
        small_net_inflow: f64_to_dec(c.small_net_inflow),
        medium_net_inflow: f64_to_dec(c.medium_net_inflow),
        large_net_inflow: f64_to_dec(c.large_net_inflow),
        super_large_net_inflow: f64_to_dec(c.super_large_net_inflow),
        main_net_inflow_ratio_pct: c.main_net_inflow_ratio_pct,
        small_net_inflow_ratio_pct: c.small_net_inflow_ratio_pct,
        medium_net_inflow_ratio_pct: c.medium_net_inflow_ratio_pct,
        large_net_inflow_ratio_pct: c.large_net_inflow_ratio_pct,
        super_large_net_inflow_ratio_pct: c.super_large_net_inflow_ratio_pct,
        close: f64_to_dec(c.close),
        change_pct: c.change_pct,
    }
}

pub(crate) fn news_item_from_akshare(n: akshare::NewsItem) -> NewsItem {
    NewsItem {
        published_at: n.published_at,
        title: n.title,
        summary: n.summary,
        source: n.source,
        url: n.url,
    }
}

pub(crate) fn news_item_from_announcement(a: akshare::AnnouncementItem) -> NewsItem {
    NewsItem {
        published_at: a.published_at,
        title: a.title.clone(),
        summary: a.title,
        source: a.source,
        url: a.url,
    }
}

pub(crate) fn news_item_from_stock_news(n: akshare::stock::feature::StockNews) -> NewsItem {
    NewsItem {
        published_at: n.publish_time,
        title: n.title.clone(),
        summary: n.content.unwrap_or(n.title),
        source: n.source.unwrap_or_else(|| "Eastmoney".to_string()),
        url: n.url,
    }
}

pub(crate) fn balance_sheet_to_wire(
    b: &akshare::stock::feature::BalanceSheet,
) -> super::wire::EastmoneyBalanceSheetItem {
    super::wire::EastmoneyBalanceSheetItem {
        report_date: b.notice_date.clone(),
        total_assets: b.total_assets,
        total_liabilities: b.total_liabilities,
        total_equity: b.equity,
        monetary_funds: b.cash,
        current_liab: None,
        totalnoncliab: None,
    }
}

pub(crate) fn cashflow_to_wire(
    c: &akshare::stock::feature::CashFlowSheet,
) -> super::wire::EastmoneyCashflowItem {
    super::wire::EastmoneyCashflowItem {
        report_date: c.notice_date.clone(),
        netcash_operate: c.operating_cash_flow,
        construct_long_asset: c.investing_cash_flow,
        end_cce: c.cash_increase,
    }
}

pub(crate) fn profit_sheet_to_wire(
    p: &akshare::stock::feature::ProfitSheet,
) -> super::wire::ProfitSheetWire {
    super::wire::ProfitSheetWire {
        notice_date: p.notice_date.clone(),
        total_revenue: p.total_revenue,
        net_profit: p.net_profit,
        net_profit_deducted: p.net_profit_deducted,
    }
}

pub(crate) fn quote_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:tencent_quote+eastmoney",
        MarketKind::HongKong => "akshare:tencent_quote+yahoo_finance_chart",
        MarketKind::UsEquity => "akshare:sina_us_daily+yahoo_finance_chart+stooq",
    }
}

pub(crate) fn fundamentals_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:eastmoney",
        MarketKind::HongKong => "akshare:tencent_quote+eastmoney_search",
        MarketKind::UsEquity => "akshare:sec_edgar",
    }
}

pub(crate) fn news_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:eastmoney+searxng_news",
        MarketKind::HongKong => "akshare:searxng_news",
        MarketKind::UsEquity => "akshare:sec_edgar+searxng_news",
    }
}

pub(crate) fn candles_source(market: MarketKind) -> &'static str {
    match market {
        MarketKind::AShare => "akshare:tencent_kline+eastmoney_kline",
        MarketKind::HongKong => "akshare:tencent_kline+yahoo_finance_chart",
        MarketKind::UsEquity => "akshare:sina_us_daily+yahoo_finance_chart+stooq",
    }
}

// ---------------------------------------------------------------------------
// Dispatch functions
// ---------------------------------------------------------------------------

pub(crate) async fn fetch_quote(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<ProviderResult<QuoteSnapshot>> {
    match client.detect_market(symbol) {
        MarketKind::AShare => {
            let ts_code = client
                .normalize_a_share_symbol(symbol)
                .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
            a_share::fetch_quote(client, symbol, &ts_code).await
        }
        MarketKind::HongKong => hk::fetch_quote(client, symbol).await,
        MarketKind::UsEquity => us::fetch_quote(client, symbol).await,
    }
}

pub(crate) async fn fetch_fundamentals(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<FundamentalsSnapshot> {
    match client.detect_market(symbol) {
        MarketKind::AShare => {
            let ts_code = client
                .normalize_a_share_symbol(symbol)
                .ok_or_else(|| anyhow::anyhow!("invalid A-share symbol"))?;
            a_share::fetch_fundamentals(client, symbol, &ts_code).await
        }
        MarketKind::HongKong => hk::fetch_fundamentals(client, symbol).await,
        MarketKind::UsEquity => us::fetch_fundamentals(client, symbol).await,
    }
}

pub(crate) async fn fetch_insider_transactions(
    client: &MarketDataClient,
    symbol: &str,
) -> anyhow::Result<Vec<NewsItem>> {
    match client.detect_market(symbol) {
        MarketKind::AShare => a_share::fetch_insider_transactions(client, symbol).await,
        MarketKind::HongKong => hk::fetch_insider_transactions(client, symbol).await,
        MarketKind::UsEquity => us::fetch_insider_transactions(client, symbol).await,
    }
}

pub(crate) async fn fetch_candles(
    client: &MarketDataClient,
    symbol: &str,
    adjust: &str,
    limit: usize,
) -> anyhow::Result<ProviderResult<Vec<CandlePoint>>> {
    match client.detect_market(symbol) {
        MarketKind::AShare => a_share::fetch_candles(client, symbol, adjust, limit).await,
        MarketKind::HongKong => hk::fetch_candles(client, symbol, limit).await,
        MarketKind::UsEquity => us::fetch_candles(client, symbol, limit).await,
    }
}

pub(crate) async fn search_stocks(
    client: &MarketDataClient,
    query: &str,
    market: Option<&str>,
    limit: usize,
) -> anyhow::Result<Vec<StockSearchResult>> {
    a_share::search_stocks(client, query, market, limit).await
}

pub(crate) async fn fetch_return_since(
    client: &MarketDataClient,
    symbol: &str,
    start_date: &str,
    holding_days: usize,
) -> anyhow::Result<Option<f64>> {
    match client.detect_market(symbol) {
        MarketKind::AShare => {
            a_share::fetch_return_since(client, symbol, start_date, holding_days).await
        }
        MarketKind::HongKong => {
            hk::fetch_return_since(client, symbol, start_date, holding_days).await
        }
        MarketKind::UsEquity => {
            us::fetch_return_since(client, symbol, start_date, holding_days).await
        }
    }
}

// ---------------------------------------------------------------------------
// A-share module
// ---------------------------------------------------------------------------

pub(crate) mod a_share {
    use super::ProviderResult;
    use crate::data::{CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem, QuoteSnapshot};
    use akshare::stock::feature::{
        AnalystDetail, AnalystRank, BalanceSheet, CashFlowSheet, CommentDesireIndex,
        CommentFocusIndex, CommentHistScore, CommentOrgParticipation, DividendInfo, DzjyHygtj,
        DzjyHyyybtj, DzjyMrtj, DzjyYybph, EarningsForecast as AkEarningsForecast,
        EarningsQuickReport, EarningsReport, EsgRating, FundFlowEntry,
        GdfxHoldingAnalyse, GdfxHoldingChange, GdfxHoldingDetail, GdfxHoldingStatistic,
        GdfxTeamwork, GdfxTop10, Gdhs, GdhsDetail, Ggcg, GpzyDistributeEntry, GpzyIndustry,
        GpzyPledgeDetail, GpzyPledgeRatio, GpzyPledgeRatioDetail, GpzyProfile, HotStockXq,
        IndustryCategory, JgdyDetail, JgdyTj, LhbDetail, LhbHyyyb, LhbJgmmtj, LhbJgstatistic,
        LhbStockDetail, LhbStockDetailDate, LhbStockStatistic, LhbTraderStatistic, LhbYybDetail,
        LhbYybph, MainFundFlow, MarginAccountInfo, MarginRatioPa, MarginSseDetail, MarginSseSummary,
        MarginSzseDetail, MarginSzseSummary, PankouChange, ProfitSheet,
        SectorFundFlowRank, StockComment, ZtPool, ZtPoolDtgc,
        ZtPoolPrevious, ZtPoolStrong, ZtPoolSubNew, ZtPoolZbgc,
    };
    use akshare::types::StockSearchResult;

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
        Ok((super::quote_from_akshare(ak_quote), "akshare".to_string()))
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
        Ok((ak_candles.into_iter().map(super::candle_from_akshare).collect(), "akshare".to_string()))
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

    // Fund flow
    pub(crate) async fn fetch_fund_flow_individual(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<FundFlowEntry>> {
        client.ak.stock_fund_flow_individual(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_fund_flow_concept(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<SectorFundFlowRank>> {
        client.ak.stock_fund_flow_concept(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_fund_flow_industry(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<SectorFundFlowRank>> {
        client.ak.stock_fund_flow_industry(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_main_fund_flow(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<MainFundFlow>> {
        client.ak.stock_main_fund_flow(symbol).await.map_err(Into::into)
    }

    // Billboard
    pub(crate) async fn fetch_billboard_detail(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<LhbDetail>> {
        client.ak.stock_lhb_detail_em(start_date, end_date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_stock_statistic(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<LhbStockStatistic>> {
        client.ak.stock_lhb_stock_statistic_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_jgmmtj(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<LhbJgmmtj>> {
        client.ak.stock_lhb_jgmmtj_em(start_date, end_date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_jgstatistic(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<LhbJgstatistic>> {
        client.ak.stock_lhb_jgstatistic_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_hyyyb(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<LhbHyyyb>> {
        client.ak.stock_lhb_hyyyb_em(start_date, end_date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_yybph(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<LhbYybph>> {
        client.ak.stock_lhb_yybph_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_trader_statistic(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<LhbTraderStatistic>> {
        client.ak.stock_lhb_traderstatistic_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_stock_detail_date(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<LhbStockDetailDate>> {
        client.ak.stock_lhb_stock_detail_date_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_stock_detail(client: &MarketDataClient, symbol: &str, date: &str, flag: &str) -> anyhow::Result<Vec<LhbStockDetail>> {
        client.ak.stock_lhb_stock_detail_em(symbol, date, flag).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_billboard_yyb_detail(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<LhbYybDetail>> {
        client.ak.stock_lhb_yyb_detail_em(symbol).await.map_err(Into::into)
    }

    // Margin
    pub(crate) async fn fetch_margin_account_info(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<MarginAccountInfo>> {
        client.ak.stock_margin_account_info_em(start_date, end_date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_margin_sse_detail(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<MarginSseDetail>> {
        client.ak.stock_margin_detail_sse(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_margin_szse_detail(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<MarginSzseDetail>> {
        client.ak.stock_margin_detail_szse(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_margin_ratio_pa(client: &MarketDataClient, symbol: &str, date: &str) -> anyhow::Result<Vec<MarginRatioPa>> {
        client.ak.stock_margin_ratio_pa(symbol, date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_margin_sse_summary(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<MarginSseSummary>> {
        client.ak.stock_margin_sse(start_date, end_date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_margin_szse_summary(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<MarginSzseSummary>> {
        client.ak.stock_margin_szse(date).await.map_err(Into::into)
    }

    // Zt Pool
    pub(crate) async fn fetch_zt_pool(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<ZtPool>> {
        client.ak.stock_zt_pool_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_zt_pool_dtgc(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<ZtPoolDtgc>> {
        client.ak.stock_zt_pool_dtgc_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_zt_pool_previous(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<ZtPoolPrevious>> {
        client.ak.stock_zt_pool_previous_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_zt_pool_strong(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<ZtPoolStrong>> {
        client.ak.stock_zt_pool_strong_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_zt_pool_sub_new(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<ZtPoolSubNew>> {
        client.ak.stock_zt_pool_sub_new_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_zt_pool_zbgc(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<ZtPoolZbgc>> {
        client.ak.stock_zt_pool_zbgc_em(date).await.map_err(Into::into)
    }

    // Earnings
    pub(crate) async fn fetch_earnings_forecast(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<AkEarningsForecast>> {
        client.ak.stock_yjyg_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_earnings_quick_report(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<EarningsQuickReport>> {
        client.ak.stock_yjkb_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_earnings_report(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<EarningsReport>> {
        client.ak.stock_yjbb_em(date).await.map_err(Into::into)
    }

    // Analyst
    pub(crate) async fn fetch_analyst_rank(client: &MarketDataClient, year: &str) -> anyhow::Result<Vec<AnalystRank>> {
        client.ak.stock_analyst_rank_em(year).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_analyst_detail(client: &MarketDataClient, analyst_id: &str, indicator: &str) -> anyhow::Result<Vec<AnalystDetail>> {
        client.ak.stock_analyst_detail_em(analyst_id, indicator).await.map_err(Into::into)
    }

    // Shareholder Analysis
    pub(crate) async fn fetch_gdfx_free_holding_statistics(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<GdfxHoldingStatistic>> {
        client.ak.stock_gdfx_free_holding_statistics_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_holding_statistics(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<GdfxHoldingStatistic>> {
        client.ak.stock_gdfx_holding_statistics_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_free_holding_change(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<GdfxHoldingChange>> {
        client.ak.stock_gdfx_free_holding_change_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_holding_change(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<GdfxHoldingChange>> {
        client.ak.stock_gdfx_holding_change_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_free_top10(client: &MarketDataClient, symbol: &str, date: &str) -> anyhow::Result<Vec<GdfxTop10>> {
        client.ak.stock_gdfx_free_top_10_em(symbol, date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_top10(client: &MarketDataClient, symbol: &str, date: &str) -> anyhow::Result<Vec<GdfxTop10>> {
        client.ak.stock_gdfx_top_10_em(symbol, date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_free_holding_detail(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<GdfxHoldingDetail>> {
        client.ak.stock_gdfx_free_holding_detail_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_holding_detail(client: &MarketDataClient, date: &str, indicator: &str, symbol: &str) -> anyhow::Result<Vec<GdfxHoldingDetail>> {
        client.ak.stock_gdfx_holding_detail_em(date, indicator, symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_free_holding_analyse(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<GdfxHoldingAnalyse>> {
        client.ak.stock_gdfx_free_holding_analyse_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_holding_analyse(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<GdfxHoldingAnalyse>> {
        client.ak.stock_gdfx_holding_analyse_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_free_teamwork(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<GdfxTeamwork>> {
        client.ak.stock_gdfx_free_holding_teamwork_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_gdfx_teamwork(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<GdfxTeamwork>> {
        client.ak.stock_gdfx_holding_teamwork_em(symbol).await.map_err(Into::into)
    }

    // Block Trades
    pub(crate) async fn fetch_block_trade_daily(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<DzjyMrtj>> {
        client.ak.stock_dzjy_mrtj(start_date, end_date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_block_trade_industry(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<DzjyHygtj>> {
        client.ak.stock_dzjy_hygtj(start_date, end_date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_block_trade_industry_daily(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<DzjyHyyybtj>> {
        client.ak.stock_dzjy_hyyybtj(start_date, end_date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_block_trade_seat_ranking(client: &MarketDataClient, start_date: &str, end_date: &str) -> anyhow::Result<Vec<DzjyYybph>> {
        client.ak.stock_dzjy_yybph(start_date, end_date).await.map_err(Into::into)
    }

    // Hot Stocks
    pub(crate) async fn fetch_hot_follow_xq(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<HotStockXq>> {
        client.ak.stock_hot_follow_xq(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hot_tweet_xq(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<HotStockXq>> {
        client.ak.stock_hot_tweet_xq(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hot_deal_xq(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<HotStockXq>> {
        client.ak.stock_hot_deal_xq(symbol).await.map_err(Into::into)
    }

    // Order Book
    pub(crate) async fn fetch_pankou_changes(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<PankouChange>> {
        client.ak.stock_changes_em(symbol).await.map_err(Into::into)
    }

    // Dividends
    pub(crate) async fn fetch_dividends(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<DividendInfo>> {
        client.ak.stock_fhps_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_dividend_detail(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<DividendInfo>> {
        client.ak.stock_fhps_detail_em(symbol).await.map_err(Into::into)
    }

    // Pledge
    pub(crate) async fn fetch_pledge_profile(client: &MarketDataClient) -> anyhow::Result<Vec<GpzyProfile>> {
        client.ak.stock_gpzy_profile_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_pledge_ratio(client: &MarketDataClient) -> anyhow::Result<Vec<GpzyPledgeRatio>> {
        client.ak.stock_gpzy_pledge_ratio_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_pledge_detail(client: &MarketDataClient) -> anyhow::Result<Vec<GpzyPledgeDetail>> {
        client.ak.stock_gpzy_pledge_detail_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_pledge_ratio_detail(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<GpzyPledgeRatioDetail>> {
        client.ak.stock_gpzy_individual_pledge_ratio_detail_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_pledge_distribute_bank(client: &MarketDataClient) -> anyhow::Result<Vec<GpzyDistributeEntry>> {
        client.ak.stock_gpzy_distribute_statistics_bank_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_pledge_distribute_company(client: &MarketDataClient) -> anyhow::Result<Vec<GpzyDistributeEntry>> {
        client.ak.stock_gpzy_distribute_statistics_company_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_pledge_industry(client: &MarketDataClient) -> anyhow::Result<Vec<GpzyIndustry>> {
        client.ak.stock_gpzy_industry_data_em().await.map_err(Into::into)
    }

    // Institutional Research
    pub(crate) async fn fetch_institutional_research(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<JgdyTj>> {
        client.ak.stock_jgdy_tj_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_institutional_research_detail(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<JgdyDetail>> {
        client.ak.stock_jgdy_detail_em(date).await.map_err(Into::into)
    }

    // ESG
    pub(crate) async fn fetch_esg_msci(client: &MarketDataClient) -> anyhow::Result<Vec<EsgRating>> {
        client.ak.stock_esg_msci_sina().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_esg_rft(client: &MarketDataClient) -> anyhow::Result<Vec<EsgRating>> {
        client.ak.stock_esg_rft_sina().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_esg_zd(client: &MarketDataClient) -> anyhow::Result<Vec<EsgRating>> {
        client.ak.stock_esg_zd_sina().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_esg_hz(client: &MarketDataClient) -> anyhow::Result<Vec<EsgRating>> {
        client.ak.stock_esg_hz_sina().await.map_err(Into::into)
    }

    // Financial Reports
    pub(crate) async fn fetch_balance_sheet(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<BalanceSheet>> {
        client.ak.stock_zcfz_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_profit_sheet(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<ProfitSheet>> {
        client.ak.stock_lrb_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_cash_flow_sheet(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<CashFlowSheet>> {
        client.ak.stock_xjll_em(date).await.map_err(Into::into)
    }

    // Stock Comments
    pub(crate) async fn fetch_stock_comments(client: &MarketDataClient) -> anyhow::Result<Vec<StockComment>> {
        client.ak.stock_comment_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_comment_org_participation(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<CommentOrgParticipation>> {
        client.ak.stock_comment_detail_zlkp_jgcyd_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_comment_hist_score(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<CommentHistScore>> {
        client.ak.stock_comment_detail_zhpj_lspf_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_comment_focus_index(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<CommentFocusIndex>> {
        client.ak.stock_comment_detail_scrd_focus_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_comment_desire_index(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<CommentDesireIndex>> {
        client.ak.stock_comment_detail_scrd_desire_em(symbol).await.map_err(Into::into)
    }

    // Shareholder Changes
    pub(crate) async fn fetch_executive_shareholding(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<Ggcg>> {
        client.ak.stock_ggcg_em(symbol).await.map_err(Into::into)
    }

    // Shareholder Count
    pub(crate) async fn fetch_shareholder_count(client: &MarketDataClient, date: &str) -> anyhow::Result<Vec<Gdhs>> {
        client.ak.stock_gdhs_em(date).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_shareholder_count_detail(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<GdhsDetail>> {
        client.ak.stock_gdhs_detail_em(symbol).await.map_err(Into::into)
    }

    // Industry
    pub(crate) async fn fetch_industry_category(client: &MarketDataClient) -> anyhow::Result<Vec<IndustryCategory>> {
        client.ak.stock_industry_category_cninfo().await.map_err(Into::into)
    }

    // Global News
    pub(crate) async fn fetch_global_news_cls(client: &MarketDataClient) -> anyhow::Result<Vec<crate::types::NewsItem>> {
        let entries = client.ak.stock_info_global_cls().await?;
        Ok(entries.into_iter().map(|e| crate::types::NewsItem {
            published_at: e.time,
            title: e.title.clone(),
            summary: e.summary.unwrap_or(e.title),
            source: "CLS 财联社".to_string(),
            url: e.url,
        }).collect())
    }
    pub(crate) async fn fetch_global_news_ths(client: &MarketDataClient) -> anyhow::Result<Vec<crate::types::NewsItem>> {
        let entries = client.ak.stock_info_global_ths().await?;
        Ok(entries.into_iter().map(|e| crate::types::NewsItem {
            published_at: e.time,
            title: e.title.clone(),
            summary: e.summary.unwrap_or(e.title),
            source: "THS 同花顺".to_string(),
            url: e.url,
        }).collect())
    }
    pub(crate) async fn fetch_global_news_sina(client: &MarketDataClient) -> anyhow::Result<Vec<crate::types::NewsItem>> {
        let entries = client.ak.stock_info_global_sina().await?;
        Ok(entries.into_iter().map(|e| crate::types::NewsItem {
            published_at: e.time,
            title: e.title.clone(),
            summary: e.summary.unwrap_or(e.title),
            source: "Sina 新浪".to_string(),
            url: e.url,
        }).collect())
    }
    pub(crate) async fn fetch_global_news_futu(client: &MarketDataClient) -> anyhow::Result<Vec<crate::types::NewsItem>> {
        let entries = client.ak.stock_info_global_futu().await?;
        Ok(entries.into_iter().map(|e| crate::types::NewsItem {
            published_at: e.time,
            title: e.title.clone(),
            summary: e.summary.unwrap_or(e.title),
            source: "Futu 富途".to_string(),
            url: e.url,
        }).collect())
    }
}

// ---------------------------------------------------------------------------
// HK module
// ---------------------------------------------------------------------------

pub(crate) mod hk {
    use super::ProviderResult;
    use crate::data::{
        CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem, QuoteSnapshot,
    };
    use akshare::stock::hk_extra::{
        HkFamousStock, HkFhpxDetailThs, HkGxlLg, HkHotRank, HkHotRankDetail, HkSpotQuote,
        HkValuationBaidu,
    };
    use akshare::stock::xueqiu::XqStockSpot;

    pub(crate) async fn fetch_quote(client: &MarketDataClient, symbol: &str) -> anyhow::Result<ProviderResult<QuoteSnapshot>> {
        let ak_quote = client.ak.hk_quote(symbol).await?;
        Ok((super::quote_from_akshare(ak_quote), "akshare".to_string()))
    }
    pub(crate) async fn fetch_fundamentals(client: &MarketDataClient, symbol: &str) -> anyhow::Result<FundamentalsSnapshot> {
        client.fetch_hk_fundamentals(symbol).await
    }
    pub(crate) async fn fetch_insider_transactions(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<NewsItem>> {
        client.fetch_hk_news(symbol, 8, None, None).await
    }
    pub(crate) async fn fetch_candles(client: &MarketDataClient, symbol: &str, limit: usize) -> anyhow::Result<ProviderResult<Vec<CandlePoint>>> {
        let ak_candles = client.ak.hk_candles(symbol, limit).await?;
        Ok((ak_candles.into_iter().map(super::candle_from_akshare).collect(), "akshare".to_string()))
    }
    pub(crate) async fn fetch_return_since(client: &MarketDataClient, symbol: &str, start_date: &str, holding_days: usize) -> anyhow::Result<Option<f64>> {
        match client.fetch_hk_return_since(symbol, start_date, holding_days).await {
            Ok(value) => Ok(value),
            Err(error) => {
                tracing::info!(symbol = %symbol, start_date = %start_date, holding_days, error = ?error, "HK return_since upstream unavailable, skipping outcome computation");
                Ok(None)
            }
        }
    }
    pub(crate) async fn fetch_hk_spot(client: &MarketDataClient) -> anyhow::Result<Vec<HkSpotQuote>> {
        client.ak.stock_hk_spot().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_famous_spot(client: &MarketDataClient) -> anyhow::Result<Vec<HkFamousStock>> {
        client.ak.stock_hk_famous_spot_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_hot_rank(client: &MarketDataClient) -> anyhow::Result<Vec<HkHotRank>> {
        client.ak.stock_hk_hot_rank_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_hot_rank_latest(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<HkHotRankDetail>> {
        client.ak.stock_hk_hot_rank_latest_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_hot_rank_detail(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<HkHotRankDetail>> {
        client.ak.stock_hk_hot_rank_detail_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_hot_rank_realtime(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<HkHotRankDetail>> {
        client.ak.stock_hk_hot_rank_detail_realtime_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_dividend_payout(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        client.ak.stock_hk_dividend_payout_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_fhpx_detail(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<HkFhpxDetailThs>> {
        client.ak.stock_hk_fhpx_detail_ths(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_dividend_yield(client: &MarketDataClient) -> anyhow::Result<Vec<HkGxlLg>> {
        client.ak.stock_hk_gxl_lg().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_financial_indicators(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        client.ak.stock_hk_financial_indicator_em(symbol).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_hk_valuation(client: &MarketDataClient, symbol: &str, indicator: &str, period: &str) -> anyhow::Result<Vec<HkValuationBaidu>> {
        client.ak.stock_hk_valuation_baidu(symbol, indicator, period).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_xq_spot(client: &MarketDataClient, symbol: &str) -> anyhow::Result<XqStockSpot> {
        client.ak.stock_individual_spot_xq(symbol).await.map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// US module
// ---------------------------------------------------------------------------

pub(crate) mod us {
    use super::ProviderResult;
    use crate::data::{
        CandlePoint, FundamentalsSnapshot, MarketDataClient, NewsItem, QuoteSnapshot,
    };
    use akshare::stock::us_extra::{UsFamousStock, UsPinkStock, UsSpotSina, UsValuationBaidu};
    use akshare::stock::xueqiu::XqStockSpot;

    pub(crate) async fn fetch_quote(client: &MarketDataClient, symbol: &str) -> anyhow::Result<ProviderResult<QuoteSnapshot>> {
        let ak_quote = client.ak.us_quote(symbol).await?;
        Ok((super::quote_from_akshare(ak_quote), "akshare".to_string()))
    }
    pub(crate) async fn fetch_fundamentals(client: &MarketDataClient, symbol: &str) -> anyhow::Result<FundamentalsSnapshot> {
        client.fetch_us_fundamentals(symbol).await
    }
    pub(crate) async fn fetch_insider_transactions(client: &MarketDataClient, symbol: &str) -> anyhow::Result<Vec<NewsItem>> {
        client.fetch_us_insider_transactions(symbol).await
    }
    pub(crate) async fn fetch_candles(client: &MarketDataClient, symbol: &str, limit: usize) -> anyhow::Result<ProviderResult<Vec<CandlePoint>>> {
        let ak_candles = client.ak.us_candles(symbol, limit).await?;
        Ok((ak_candles.into_iter().map(super::candle_from_akshare).collect(), "akshare".to_string()))
    }
    pub(crate) async fn fetch_return_since(client: &MarketDataClient, symbol: &str, start_date: &str, holding_days: usize) -> anyhow::Result<Option<f64>> {
        match client.fetch_us_return_since(symbol, start_date, holding_days).await {
            Ok(value) => Ok(value),
            Err(error) => {
                tracing::info!(symbol = %symbol, start_date = %start_date, holding_days, error = ?error, "US return_since upstream unavailable, skipping outcome computation");
                Ok(None)
            }
        }
    }
    pub(crate) async fn fetch_us_spot(client: &MarketDataClient) -> anyhow::Result<Vec<UsSpotSina>> {
        client.ak.stock_us_spot().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_us_famous_spot(client: &MarketDataClient, category: &str) -> anyhow::Result<Vec<UsFamousStock>> {
        client.ak.stock_us_famous_spot_em(category).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_us_pink_spot(client: &MarketDataClient) -> anyhow::Result<Vec<UsPinkStock>> {
        client.ak.stock_us_pink_spot_em().await.map_err(Into::into)
    }
    pub(crate) async fn fetch_us_valuation(client: &MarketDataClient, symbol: &str, indicator: &str, period: &str) -> anyhow::Result<Vec<UsValuationBaidu>> {
        client.ak.stock_us_valuation_baidu(symbol, indicator, period).await.map_err(Into::into)
    }
    pub(crate) async fn fetch_xq_spot(client: &MarketDataClient, symbol: &str) -> anyhow::Result<XqStockSpot> {
        client.ak.stock_individual_spot_xq(symbol).await.map_err(Into::into)
    }
}
