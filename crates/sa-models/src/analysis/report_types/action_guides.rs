
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DirectionBreakdown {
    #[serde(default)]
    pub market: SignedScoreDimension,
    #[serde(default)]
    pub fundamentals: SignedScoreDimension,
    #[serde(default)]
    pub news: SignedScoreDimension,
    #[serde(default)]
    pub sentiment: SignedScoreDimension,
    #[serde(default)]
    pub risk_adjustment: SignedScoreDimension,
    #[serde(default)]
    pub total_score: i32,
    #[serde(default, deserialize_with = "deserialize_local_text_or_string")]
    pub implied_rating: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SignedScoreDimension {
    pub score: i32,
    pub min_score: i32,
    pub max_score: i32,
    pub rationale: LocalText,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActionBreakdown {
    #[serde(default)]
    pub alignment: ScoreDimension,
    #[serde(default)]
    pub execution_levels: ScoreDimension,
    #[serde(default)]
    pub sizing_discipline: ScoreDimension,
    #[serde(default)]
    pub horizon_clarity: ScoreDimension,
    #[serde(default)]
    pub reward_to_risk: ScoreDimension,
    #[serde(default)]
    pub total_score: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportSection {
    pub key: String,
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportActionGuides {
    #[serde(default)]
    pub holders: AudienceActionGuide,
    #[serde(default)]
    pub buyers: AudienceActionGuide,
    #[serde(default)]
    pub watchers: AudienceActionGuide,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AudienceActionGuide {
    #[serde(default)]
    pub audience: LocalText,
    #[serde(default)]
    pub user_state: LocalText,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub stance: LocalText,
    #[serde(default, skip_serializing)]
    pub summary: LocalText,
    #[serde(default, skip_serializing)]
    pub principle: LocalText,
    #[serde(default)]
    pub entry_reference: String,
    #[serde(default)]
    pub invalidation_reference: String,
    #[serde(default)]
    pub target_reference: String,
    #[serde(default)]
    pub confirmation_reference: String,
    #[serde(default)]
    pub time_horizon: String,
    #[serde(default)]
    pub sizing_reference: LocalText,
    #[serde(default)]
    pub actions: Vec<LocalText>,
    #[serde(default)]
    pub avoid: Vec<LocalText>,
    #[serde(default)]
    pub review_points: Vec<LocalText>,
    #[serde(default)]
    pub scenario_paths: Vec<ActionScenarioPath>,
}
