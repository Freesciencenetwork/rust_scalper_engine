use anyhow::{Context, Result};

use super::report::RustyFishDailyReport;

pub fn parse_rustyfish_report_json(payload: &str) -> Result<RustyFishDailyReport> {
    serde_json::from_str(payload).context("failed to parse RustyFish report JSON")
}
