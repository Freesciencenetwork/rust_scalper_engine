//! Bundled **BTC/USD 1-minute** CSV used when the client sends [`BundledBtcUsd1m`] instead of posting
//! `candles`.
//!
//! Default file: `src/historical_data/btcusd_1-min_data.csv` under the crate root. Override with env
//! **`BTCUSD_1M_CSV`** (absolute or relative path).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::Candle;

/// Load a slice of the bundled **BTC/USD 1m** CSV shipped with the repo (or path from **`BTCUSD_1M_CSV`**).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundledBtcUsd1m {
    /// Inclusive UTC calendar day **`YYYY-MM-DD`**. Omit with **`all: true`**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Inclusive UTC calendar day **`YYYY-MM-DD`**. Omit for “through end of file”.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Read the CSV oldest-first, stopping at [`MAX_BUNDLED_1M_BARS`] rows (longer files are truncated); do not set **`from`**/**`to`**.
    #[serde(default)]
    pub all: bool,
}

/// Hard cap on rows loaded per request (memory + `PreparedDataset::build` cost).
pub const MAX_BUNDLED_1M_BARS: usize = 500_000;

fn default_csv_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/historical_data/btcusd_1-min_data.csv")
}

pub fn resolve_btcusd_1m_csv_path() -> PathBuf {
    std::env::var("BTCUSD_1M_CSV").map_or_else(|_| default_csv_path(), PathBuf::from)
}

const fn utc_day_start(d: NaiveDate) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(d.and_hms_opt(0, 0, 0).expect("midnight"), Utc)
}

fn parse_day(s: &str) -> anyhow::Result<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|e| anyhow!("invalid date {s:?}: {e}"))
}

/// Inclusive `to` calendar day in UTC → exclusive upper bound (start of next UTC day after `to`).
const fn day_after_exclusive(d: NaiveDate) -> DateTime<Utc> {
    utc_day_start(d.succ_opt().expect("NaiveDate succ"))
}

#[allow(clippy::cast_possible_truncation)] // Bundled CSV uses second-resolution unix times in i64 range
fn row_ts_sec(ts_field: &str) -> anyhow::Result<i64> {
    let v: f64 = ts_field
        .trim()
        .parse()
        .with_context(|| format!("timestamp {ts_field:?}"))?;
    Ok(v as i64)
}

fn bundle_time_bounds(bundle: &BundledBtcUsd1m) -> anyhow::Result<(Option<i64>, Option<i64>)> {
    if bundle.all && (bundle.from.is_some() || bundle.to.is_some()) {
        anyhow::bail!("bundled_btcusd_1m: do not set `from`/`to` together with `all: true`");
    }
    if !bundle.all && bundle.from.is_none() && bundle.to.is_none() {
        anyhow::bail!(
            "bundled_btcusd_1m: set `from` and/or `to` as YYYY-MM-DD (UTC calendar days), or `all: true`"
        );
    }

    let lower_sec: Option<i64> = if bundle.all {
        None
    } else {
        bundle
            .from
            .as_ref()
            .map(|s| parse_day(s).map(|d| utc_day_start(d).timestamp()))
            .transpose()?
    };
    let upper_excl_sec: Option<i64> = if bundle.all {
        None
    } else {
        bundle
            .to
            .as_ref()
            .map(|s| parse_day(s).map(|d| day_after_exclusive(d).timestamp()))
            .transpose()?
    };

    if let (Some(lo), Some(hi)) = (lower_sec, upper_excl_sec)
        && hi <= lo
    {
        anyhow::bail!("bundled_btcusd_1m: `to` day must be on or after `from` day");
    }

    Ok((lower_sec, upper_excl_sec))
}

fn parse_csv_candle_line(line: &str, lineno: usize) -> anyhow::Result<(i64, Candle)> {
    let mut parts = line.splitn(6, ',');
    let ts_s = parts.next().context("timestamp column")?;
    let open = parts.next().context("open")?;
    let high = parts.next().context("high")?;
    let low = parts.next().context("low")?;
    let close = parts.next().context("close")?;
    let vol = parts.next().context("volume")?;

    let ts = row_ts_sec(ts_s)?;
    let close_time = Utc
        .timestamp_opt(ts, 0)
        .single()
        .ok_or_else(|| anyhow!("bad unix timestamp {ts}"))?;
    let candle = Candle {
        close_time,
        open: open
            .trim()
            .parse()
            .with_context(|| format!("open line {}", lineno + 2))?,
        high: high.trim().parse().context("high")?,
        low: low.trim().parse().context("low")?,
        close: close.trim().parse().context("close")?,
        volume: vol.trim().parse().context("volume")?,
        buy_volume: None,
        sell_volume: None,
        delta: None,
    };
    Ok((ts, candle))
}

/// Load filtered rows from the bundled CSV into [`Candle`]s (oldest first).
pub fn load_btcusd_1m(bundle: &BundledBtcUsd1m) -> anyhow::Result<Vec<Candle>> {
    load_btcusd_1m_from_path(&resolve_btcusd_1m_csv_path(), bundle)
}

pub fn load_btcusd_1m_from_path(
    path: &Path,
    bundle: &BundledBtcUsd1m,
) -> anyhow::Result<Vec<Candle>> {
    let (lower_sec, upper_excl_sec) = bundle_time_bounds(bundle)?;

    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut lines = BufReader::new(file).lines();
    let header = lines
        .next()
        .transpose()
        .context("read csv header")?
        .ok_or_else(|| anyhow!("empty csv"))?;
    if !header.to_lowercase().contains("timestamp") {
        anyhow::bail!("unexpected csv header: {header:?}");
    }

    let mut out: Vec<Candle> = Vec::new();
    for (lineno, line) in lines.enumerate() {
        let line = line.with_context(|| format!("line {}", lineno + 2))?;
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        let (ts, candle) = parse_csv_candle_line(line, lineno)?;

        if let Some(lo) = lower_sec
            && ts < lo
        {
            continue;
        }
        if let Some(hi) = upper_excl_sec
            && ts >= hi
        {
            break;
        }

        out.push(candle);

        if out.len() > MAX_BUNDLED_1M_BARS {
            anyhow::bail!(
                "bundled BTC/USD 1m slice exceeds {MAX_BUNDLED_1M_BARS} rows; narrow `from`/`to` or increase cap in code"
            );
        }
        if bundle.all && out.len() == MAX_BUNDLED_1M_BARS {
            tracing::warn!(
                max_rows = MAX_BUNDLED_1M_BARS,
                csv_path = %path.display(),
                "`bundled_btcusd_1m.all`: stopping read at row cap (file may have more rows)"
            );
            break;
        }
    }

    if out.is_empty() {
        anyhow::bail!(
            "no rows in range for {} (check dates and file coverage)",
            path.display()
        );
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn tiny_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/btcusd_1m_tiny.csv")
    }

    #[test]
    fn loads_fixture_all() {
        let path = tiny_fixture();
        let bundle = BundledBtcUsd1m {
            from: None,
            to: None,
            all: true,
        };
        let v = load_btcusd_1m_from_path(&path, &bundle).expect("load");
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn loads_fixture_day_slice() {
        let path = tiny_fixture();
        let bundle = BundledBtcUsd1m {
            from: Some("2000-01-01".to_string()),
            to: Some("2030-01-01".to_string()),
            all: false,
        };
        let v = load_btcusd_1m_from_path(&path, &bundle).expect("load");
        assert_eq!(v.len(), 4);
    }
}
