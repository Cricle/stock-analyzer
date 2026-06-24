impl std::fmt::Display for DecisionTargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            DecisionTargetType::Point => "point",
            DecisionTargetType::Range => "range",
            DecisionTargetType::Conditional => "conditional",
            DecisionTargetType::Open => "open",
            DecisionTargetType::Unknown => "unknown",
        };
        write!(f, "{value}")
    }
}
