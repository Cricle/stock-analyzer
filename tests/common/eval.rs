use stock_analyzer::{CandlePoint, FundamentalsSnapshot, NewsItem, QuoteSnapshot};

pub struct StockEvalResult {
    pub symbol: String,
    pub name: String,
    pub quote_ok: bool,
    pub fundamentals_ok: bool,
    pub fundamentals_partial: bool,
    pub news_ok: bool,
    pub news_count: usize,
    pub candles_ok: bool,
    pub candle_count: usize,
}

impl StockEvalResult {
    pub fn score_pct(&self) -> u32 {
        let mut total = 0;
        let mut max = 0;

        max += 25;
        if self.quote_ok {
            total += 25;
        }

        max += 25;
        if self.fundamentals_ok {
            total += 25;
        } else if self.fundamentals_partial {
            total += 15;
        }

        max += 25;
        if self.news_ok {
            total += 25;
        } else if self.news_count > 0 {
            total += 10;
        }

        max += 25;
        if self.candles_ok {
            total += 25;
        } else if self.candle_count > 0 {
            total += 10;
        }

        (total * 100) / max
    }
}

pub fn assert_quote_valid(quote: &QuoteSnapshot) -> bool {
    let price_ok = quote.close > 0.0;
    let volume_ok = quote.volume > 0;
    price_ok && volume_ok
}

pub fn assert_fundamentals_valid(fund: &FundamentalsSnapshot) -> (bool, bool) {
    let has_metric = fund.net_income_usd.is_some()
        || fund.revenues_usd.is_some()
        || fund.stockholders_equity_usd.is_some();
    let has_market_cap = fund.market_cap.is_some() && fund.market_cap.unwrap() > 0.0;
    let is_complete = has_metric && has_market_cap && !fund.company_name.is_empty();
    let is_partial = has_metric || has_market_cap;
    (is_complete, is_partial)
}

pub fn assert_news_valid(news: &[NewsItem]) -> bool {
    if news.len() < 3 {
        return false;
    }
    news.iter().all(|n| !n.title.is_empty())
}

pub fn assert_candles_valid(candles: &[CandlePoint]) -> bool {
    if candles.len() < 60 {
        return false;
    }
    candles
        .iter()
        .all(|c| c.open > 0.0 && c.close > 0.0 && c.high >= c.low && c.volume > 0)
}

pub fn print_completeness_table(results: &[StockEvalResult]) {
    println!(
        "\n{:<10} | {:<8} | {:<15} | {:<8} | {:<10} | {}",
        "Stock", "Quote", "Fundamentals", "News", "Candles", "Score"
    );
    println!("{}", "-".repeat(75));
    for r in results {
        let quote = if r.quote_ok { "OK" } else { "MISSING" };
        let fund = if r.fundamentals_ok {
            "OK"
        } else if r.fundamentals_partial {
            "partial"
        } else {
            "MISSING"
        };
        let news = if r.news_ok {
            "OK"
        } else if r.news_count > 0 {
            &format!("{} items", r.news_count)
        } else {
            "MISSING"
        };
        let candles = if r.candles_ok {
            "OK"
        } else if r.candle_count > 0 {
            &format!("{} days", r.candle_count)
        } else {
            "MISSING"
        };
        println!(
            "{:<10} | {:<8} | {:<15} | {:<8} | {:<10} | {}%",
            r.symbol,
            quote,
            fund,
            news,
            candles,
            r.score_pct()
        );
    }
    let avg = results.iter().map(|r| r.score_pct() as f64).sum::<f64>() / results.len() as f64;
    println!("{}", "-".repeat(75));
    println!(
        "{:<10} | {:<8} | {:<15} | {:<8} | {:<10} | {:.0}%",
        "AVERAGE", "", "", "", "", avg
    );
}
