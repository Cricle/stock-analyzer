use sa_data::MarketKind;

pub struct TestStock {
    pub symbol: &'static str,
    pub name: &'static str,
    pub market: &'static str,
    pub market_kind: MarketKind,
    pub is_famous: bool,
}

pub const TEST_STOCKS: &[TestStock] = &[
    TestStock {
        symbol: "600519",
        name: "贵州茅台",
        market: "A股",
        market_kind: MarketKind::AShare,
        is_famous: true,
    },
    TestStock {
        symbol: "688256",
        name: "寒武纪",
        market: "A股",
        market_kind: MarketKind::AShare,
        is_famous: false,
    },
    TestStock {
        symbol: "00700",
        name: "腾讯控股",
        market: "港股",
        market_kind: MarketKind::HongKong,
        is_famous: true,
    },
    TestStock {
        symbol: "00020",
        name: "商汤科技",
        market: "港股",
        market_kind: MarketKind::HongKong,
        is_famous: false,
    },
    TestStock {
        symbol: "AAPL",
        name: "Apple",
        market: "美股",
        market_kind: MarketKind::UsEquity,
        is_famous: true,
    },
    TestStock {
        symbol: "PLTR",
        name: "Palantir",
        market: "美股",
        market_kind: MarketKind::UsEquity,
        is_famous: false,
    },
];

pub fn a_share_stocks() -> Vec<&'static TestStock> {
    TEST_STOCKS.iter().filter(|s| s.market_kind == MarketKind::AShare).collect()
}

pub fn hk_stocks() -> Vec<&'static TestStock> {
    TEST_STOCKS.iter().filter(|s| s.market_kind == MarketKind::HongKong).collect()
}

pub fn us_stocks() -> Vec<&'static TestStock> {
    TEST_STOCKS.iter().filter(|s| s.market_kind == MarketKind::UsEquity).collect()
}
