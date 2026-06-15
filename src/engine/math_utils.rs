//! Mathematical utility functions for continuous scoring.
//!
//! Used by both stock pick objective assessment and report scoring systems.

/// Sigmoid compression: maps any value to 0-1 range.
/// `k` controls steepness (0.5 = gentle, 1.0 = steep).
pub fn sigmoid(x: f64, center: f64, k: f64) -> f64 {
    1.0 / (1.0 + (-k * (x - center)).exp())
}

/// Z-Score standardization: (x - mean) / std.
pub fn z_score(x: f64, mean: f64, std_dev: f64) -> f64 {
    if std_dev.abs() < 1e-10 {
        0.0
    } else {
        (x - mean) / std_dev
    }
}

/// Percentile rank: 0-100.
pub fn percentile_rank(value: f64, all_values: &[f64]) -> f64 {
    let count_below = all_values.iter().filter(|&&v| v < value).count();
    count_below as f64 / all_values.len().max(1) as f64 * 100.0
}

/// Piecewise linear interpolation between breakpoints.
/// `breakpoints`: sorted `[(x0, y0), (x1, y1), ...]`.
/// Values outside the range are clamped to the nearest endpoint.
pub fn piecewise_linear(x: f64, breakpoints: &[(f64, f64)]) -> f64 {
    if breakpoints.is_empty() {
        return 0.0;
    }
    if breakpoints.len() == 1 {
        return breakpoints[0].1;
    }
    if x <= breakpoints[0].0 {
        return breakpoints[0].1;
    }
    if x >= breakpoints[breakpoints.len() - 1].0 {
        return breakpoints[breakpoints.len() - 1].1;
    }
    for window in breakpoints.windows(2) {
        let (x0, y0) = window[0];
        let (x1, y1) = window[1];
        if x >= x0 && x <= x1 {
            let t = if (x1 - x0).abs() < 1e-10 {
                0.0
            } else {
                (x - x0) / (x1 - x0)
            };
            return y0 + t * (y1 - y0);
        }
    }
    breakpoints[breakpoints.len() - 1].1
}

/// Exponential decay: `2^(-x / half_life)`.
/// Returns 1.0 at x=0, 0.5 at x=half_life, approaching 0 for large x.
pub fn exponential_decay(x: f64, half_life: f64) -> f64 {
    2.0_f64.powf(-x / half_life)
}

/// Deviation rate: `(value - reference) / reference`.
pub fn deviation_rate(value: f64, reference: f64) -> f64 {
    if reference.abs() < 1e-10 {
        0.0
    } else {
        (value - reference) / reference
    }
}

/// Deviation penalty: quadratic penalty when `|z| > threshold`.
/// Returns a negative value (penalty) or 0.0.
pub fn deviation_penalty(z: f64, threshold: f64) -> f64 {
    if z.abs() > threshold {
        -(z.abs() - threshold).powi(2)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sigmoid_center() {
        let v = sigmoid(5.0, 5.0, 1.0);
        assert!((v - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_sigmoid_high() {
        let v = sigmoid(10.0, 5.0, 1.0);
        assert!(v > 0.99);
    }

    #[test]
    fn test_sigmoid_low() {
        let v = sigmoid(0.0, 5.0, 1.0);
        assert!(v < 0.01);
    }

    #[test]
    fn test_z_score() {
        assert!((z_score(10.0, 5.0, 2.5) - 2.0).abs() < 1e-10);
        assert!((z_score(5.0, 5.0, 2.5)).abs() < 1e-10);
        assert_eq!(z_score(10.0, 5.0, 0.0), 0.0);
    }

    #[test]
    fn test_percentile_rank() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile_rank(3.0, &values) - 40.0).abs() < 1e-10);
        assert!((percentile_rank(1.0, &values)).abs() < 1e-10);
        assert!((percentile_rank(5.0, &values) - 80.0).abs() < 1e-10);
    }

    #[test]
    fn test_piecewise_linear() {
        let bp = [(0.0, 0.0), (10.0, 100.0), (20.0, 150.0)];
        assert!((piecewise_linear(5.0, &bp) - 50.0).abs() < 1e-10);
        assert!((piecewise_linear(15.0, &bp) - 125.0).abs() < 1e-10);
        assert!((piecewise_linear(-1.0, &bp)).abs() < 1e-10);
        assert!((piecewise_linear(25.0, &bp) - 150.0).abs() < 1e-10);
    }

    #[test]
    fn test_exponential_decay() {
        assert!((exponential_decay(0.0, 5.0) - 1.0).abs() < 1e-10);
        assert!((exponential_decay(5.0, 5.0) - 0.5).abs() < 1e-10);
        assert!(exponential_decay(10.0, 5.0) < 0.26);
    }

    #[test]
    fn test_deviation_rate() {
        assert!((deviation_rate(12.0, 10.0) - 0.2).abs() < 1e-10);
        assert!((deviation_rate(8.0, 10.0) - (-0.2)).abs() < 1e-10);
        assert_eq!(deviation_rate(5.0, 0.0), 0.0);
    }

    #[test]
    fn test_deviation_penalty() {
        assert_eq!(deviation_penalty(1.0, 2.0), 0.0);
        assert_eq!(deviation_penalty(-1.5, 2.0), 0.0);
        assert!(deviation_penalty(3.0, 2.0) < 0.0);
        assert!((deviation_penalty(3.0, 2.0) - (-1.0)).abs() < 1e-10);
    }
}
