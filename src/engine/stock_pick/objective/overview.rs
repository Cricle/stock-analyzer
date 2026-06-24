use crate::models::{StockPickItem, StockPickObjectiveBucket, StockPickObjectiveOverview};

pub(crate) fn summarize_stock_pick_objective_overview(
    picks: &[StockPickItem],
) -> StockPickObjectiveOverview {
    if picks.is_empty() {
        return StockPickObjectiveOverview::default();
    }
    let scores = picks
        .iter()
        .map(|item| item.objective_assessment.final_score)
        .collect::<Vec<_>>();
    let total = scores.iter().sum::<i32>() as f64;
    let average_score = total / scores.len() as f64;
    let buckets = [
        (
            "A",
            picks
                .iter()
                .filter(|item| item.objective_assessment.grade == "A")
                .count(),
        ),
        (
            "B",
            picks
                .iter()
                .filter(|item| item.objective_assessment.grade == "B")
                .count(),
        ),
        (
            "C",
            picks
                .iter()
                .filter(|item| item.objective_assessment.grade == "C")
                .count(),
        ),
        (
            "D",
            picks
                .iter()
                .filter(|item| item.objective_assessment.grade == "D")
                .count(),
        ),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(label, count)| StockPickObjectiveBucket {
        label: label.to_string(),
        count,
    })
    .collect::<Vec<_>>();
    StockPickObjectiveOverview {
        average_score,
        average_grade: stock_pick_objective_grade(average_score.round() as i32).to_string(),
        min_score: *scores.iter().min().unwrap_or(&0),
        max_score: *scores.iter().max().unwrap_or(&0),
        ready_picks: picks
            .iter()
            .filter(|item| item.objective_assessment.ready)
            .count(),
        incomplete_picks: picks
            .iter()
            .filter(|item| !item.objective_assessment.ready)
            .count(),
        distribution: buckets,
    }
}

fn stock_pick_objective_grade(score: i32) -> &'static str {
    match score {
        85..=100 => "A",
        75..=84 => "B",
        60..=74 => "C",
        _ => "D",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StockPickObjectiveAssessment;

    fn make_pick(score: i32, grade: &str, ready: bool) -> StockPickItem {
        StockPickItem {
            symbol: "TEST".to_string(),
            name: "Test Stock".to_string(),
            market: "A-share".to_string(),
            objective_assessment: StockPickObjectiveAssessment {
                final_score: score,
                grade: grade.to_string(),
                ready,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_stock_pick_objective_grade_a() {
        assert_eq!(stock_pick_objective_grade(90), "A");
        assert_eq!(stock_pick_objective_grade(85), "A");
        assert_eq!(stock_pick_objective_grade(100), "A");
    }

    #[test]
    fn test_stock_pick_objective_grade_b() {
        assert_eq!(stock_pick_objective_grade(80), "B");
        assert_eq!(stock_pick_objective_grade(75), "B");
        assert_eq!(stock_pick_objective_grade(84), "B");
    }

    #[test]
    fn test_stock_pick_objective_grade_c() {
        assert_eq!(stock_pick_objective_grade(70), "C");
        assert_eq!(stock_pick_objective_grade(60), "C");
        assert_eq!(stock_pick_objective_grade(74), "C");
    }

    #[test]
    fn test_stock_pick_objective_grade_d() {
        assert_eq!(stock_pick_objective_grade(50), "D");
        assert_eq!(stock_pick_objective_grade(0), "D");
        assert_eq!(stock_pick_objective_grade(-10), "D");
    }

    #[test]
    fn test_summarize_empty() {
        let overview = summarize_stock_pick_objective_overview(&[]);
        assert_eq!(overview.average_score, 0.0);
        assert_eq!(overview.distribution.len(), 0);
    }

    #[test]
    fn test_summarize_single() {
        let picks = vec![make_pick(90, "A", true)];
        let overview = summarize_stock_pick_objective_overview(&picks);
        assert!((overview.average_score - 90.0).abs() < 0.01);
        assert_eq!(overview.average_grade, "A");
        assert_eq!(overview.min_score, 90);
        assert_eq!(overview.max_score, 90);
        assert_eq!(overview.ready_picks, 1);
        assert_eq!(overview.incomplete_picks, 0);
    }

    #[test]
    fn test_summarize_multiple_grades() {
        let picks = vec![
            make_pick(90, "A", true),
            make_pick(80, "B", true),
            make_pick(65, "C", false),
            make_pick(50, "D", false),
        ];
        let overview = summarize_stock_pick_objective_overview(&picks);
        assert!((overview.average_score - 71.25).abs() < 0.01);
        assert_eq!(overview.min_score, 50);
        assert_eq!(overview.max_score, 90);
        assert_eq!(overview.ready_picks, 2);
        assert_eq!(overview.incomplete_picks, 2);
        assert_eq!(overview.distribution.len(), 4);
    }

    #[test]
    fn test_summarize_same_grade() {
        let picks = vec![make_pick(90, "A", true), make_pick(95, "A", true)];
        let overview = summarize_stock_pick_objective_overview(&picks);
        assert_eq!(overview.distribution.len(), 1);
        assert_eq!(overview.distribution[0].label, "A");
        assert_eq!(overview.distribution[0].count, 2);
    }
}
