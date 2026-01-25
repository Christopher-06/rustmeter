#[derive(Debug, Clone)]
pub struct LinearRegression {
    pub slope: f64,
    pub offset: f64,
}

impl LinearRegression {
    /// Perform linear regression on the given x and y data points
    pub fn perform_linear_regression(x: &[f64], y: &[f64]) -> Option<Self> {
        if x.len() != y.len() || x.is_empty() {
            return None;
        }

        let n = x.len() as f64;
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| xi * yi).sum();
        let sum_x2: f64 = x.iter().map(|xi| xi * xi).sum();

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < f64::EPSILON {
            return None; // Prevent division by zero
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;
        let offset = (sum_y - slope * sum_x) / n;

        Some(Self { slope, offset })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_regression() {
        const EXPECTED_SLOPE: f64 = 2.5;
        const EXPECTED_OFFSET: f64 = 1.0;

        let x: Vec<f64> = (-100..100).map(|v| v as f64).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|v| EXPECTED_SLOPE * v + EXPECTED_OFFSET)
            .collect();

        let result = LinearRegression::perform_linear_regression(&x, &y).unwrap();
        assert!((result.slope - EXPECTED_SLOPE).abs() < 1e-6);
        assert!((result.offset - EXPECTED_OFFSET).abs() < 1e-6);
    }
}
