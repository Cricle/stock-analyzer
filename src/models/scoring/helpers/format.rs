
trait StructuredNumericExt {
    fn entry_price_numeric_count(&self) -> i32;
    fn stop_loss_numeric_count(&self) -> i32;
    fn price_target_numeric_count(&self) -> i32;
}

impl StructuredNumericExt for StructuredTraderPlan {
    fn entry_price_numeric_count(&self) -> i32 {
        self.entry_price.as_str().numeric_count()
    }

    fn stop_loss_numeric_count(&self) -> i32 {
        self.stop_loss.as_str().numeric_count()
    }

    fn price_target_numeric_count(&self) -> i32 {
        0
    }
}

impl StructuredNumericExt for StructuredPortfolioDecision {
    fn entry_price_numeric_count(&self) -> i32 {
        0
    }

    fn stop_loss_numeric_count(&self) -> i32 {
        0
    }

    fn price_target_numeric_count(&self) -> i32 {
        self.price_target.as_str().numeric_count()
    }
}

#[cfg(test)]
mod format_tests;
