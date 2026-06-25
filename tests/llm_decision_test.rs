use sa::llm::prompt::ConfidenceCalibration;
use sa::llm::prompt::build_decision_framework_prompt;

#[test]
fn test_decision_framework_prompt_with_high_completeness() {
    let prompt = build_decision_framework_prompt(85.0, 90.0, 70.0, 60.0);
    assert!(prompt.contains("Data Completeness"));
    assert!(prompt.contains("85.0%"));
    assert!(prompt.contains("must give clear directional judgment"));
}

#[test]
fn test_decision_framework_prompt_with_low_completeness() {
    let prompt = build_decision_framework_prompt(30.0, 40.0, 20.0, 10.0);
    assert!(prompt.contains("30.0%"));
    assert!(prompt.contains("give Hold and explain missing data"));
}

#[test]
fn test_confidence_calibration_high_data() {
    let cal = ConfidenceCalibration::new(70.0);
    let result = cal.calibrate(0.9, 0.8, 0.7);
    assert!(result > 50.0);
    assert!(result <= 100.0);
}

#[test]
fn test_confidence_calibration_low_data() {
    let cal = ConfidenceCalibration::new(70.0);
    let result = cal.calibrate(0.3, 0.3, 0.5);
    assert!(result < 50.0);
}

#[test]
fn test_confidence_calibration_clamping() {
    let cal = ConfidenceCalibration::new(90.0);
    let result = cal.calibrate(1.5, 1.5, 1.5);
    assert!(result <= 100.0);
}
