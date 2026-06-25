use sa::guide::{DailyGuidanceRequest, GuidanceMarket};

#[test]
fn guidance_market_as_str() {
    assert_eq!(GuidanceMarket::AShare.as_str(), "a_share");
    assert_eq!(GuidanceMarket::HongKong.as_str(), "hong_kong");
    assert_eq!(GuidanceMarket::UsEquity.as_str(), "us_equity");
    assert_eq!(GuidanceMarket::All.as_str(), "all");
}

#[test]
fn guidance_market_from_str_a_share() {
    assert_eq!(GuidanceMarket::from_str("a_share"), GuidanceMarket::AShare);
    assert_eq!(GuidanceMarket::from_str("a-share"), GuidanceMarket::AShare);
    assert_eq!(GuidanceMarket::from_str("cn"), GuidanceMarket::AShare);
    assert_eq!(GuidanceMarket::from_str("ashare"), GuidanceMarket::AShare);
}

#[test]
fn guidance_market_from_str_hong_kong() {
    assert_eq!(
        GuidanceMarket::from_str("hong_kong"),
        GuidanceMarket::HongKong
    );
    assert_eq!(GuidanceMarket::from_str("hk"), GuidanceMarket::HongKong);
    assert_eq!(
        GuidanceMarket::from_str("hongkong"),
        GuidanceMarket::HongKong
    );
}

#[test]
fn guidance_market_from_str_us_equity() {
    assert_eq!(
        GuidanceMarket::from_str("us_equity"),
        GuidanceMarket::UsEquity
    );
    assert_eq!(GuidanceMarket::from_str("us"), GuidanceMarket::UsEquity);
}

#[test]
fn guidance_market_from_str_unknown() {
    assert_eq!(GuidanceMarket::from_str("unknown"), GuidanceMarket::All);
    assert_eq!(GuidanceMarket::from_str(""), GuidanceMarket::All);
}

#[test]
fn guidance_market_roundtrip() {
    let markets = [
        GuidanceMarket::AShare,
        GuidanceMarket::HongKong,
        GuidanceMarket::UsEquity,
        GuidanceMarket::All,
    ];
    for market in &markets {
        let s = market.as_str();
        let restored = GuidanceMarket::from_str(s);
        assert_eq!(*market, restored);
    }
}

#[test]
fn daily_guidance_request_market_some() {
    let req = DailyGuidanceRequest {
        market: Some("hk".to_string()),
        tickers: None,
        refresh: None,
    };
    assert_eq!(req.market(), GuidanceMarket::HongKong);
}

#[test]
fn daily_guidance_request_market_none() {
    let req = DailyGuidanceRequest {
        market: None,
        tickers: None,
        refresh: None,
    };
    assert_eq!(req.market(), GuidanceMarket::All);
}
