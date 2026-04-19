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
    /// Read the whole CSV (still capped at [`MAX_BUNDLED_1M_BARS`]); do not set **`from`**/**`to`**.
    #[serde(default)]
    pub all: bool,
}

/// Hard cap on rows loaded per request (memory + `PreparedDataset::build` cost).
pub const MAX_BUNDLED_1M_BARS: usize = 500_000;

fn default_csv_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/historical_data/btcusd_1-min_data.csv")
}

pub fn resolve_btcusd_1m_csv_path() -> PathBuf {
    std::env::var("BTCUSD_1M_CSV")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_csv_path())
}

fn utc_day_start(d: NaiveDate) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(d.and_hms_opt(0, 0, 0).expect("midnight"), Utc)
}

fn parse_day(s: &str) -> anyhow::Result<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|e| anyhow!("invalid date {s:?}: {e}"))
}

/// Inclusive `to` calendar day in UTC → exclusive upper bound (start of next UTC day after `to`).
fn day_after_exclusive(d: NaiveDate) -> DateTime<Utc> {
    utc_day_start(d.succ_opt().expect("NaiveDate succ"))
}

fn row_ts_sec(ts_field: &str) -> anyhow::Result<i64> {
    let v: f64 = ts_field
        .trim()
        .parse()
        .with_context(|| format!("timestamp {ts_field:?}"))?;
    Ok(v as i64)
}

/// Load filtered rows from the bundled CSV into [`Candle`]s (oldest first).
pub fn load_btcusd_1m(b: &BundledBtcUsd1m) -> anyhow::Result<Vec<Candle>> {
    load_btcusd_1m_from_path(&resolve_btcusd_1m_csv_path(), b)
}

pub fn load_btcusd_1m_from_path(path: &Path, b: &BundledBtcUsd1m) -> anyhow::Result<Vec<Candle>> {
    if b.all && (b.from.is_some() || b.to.is_some()) {
        anyhow::bail!("bundled_btcusd_1m: do not set `from`/`to` together with `all: true`");
    }
    if !b.all && b.from.is_none() && b.to.is_none() {
        anyhow::bail!(
            "bundled_btcusd_1m: set `from` and/or `to` as YYYY-MM-DD (UTC calendar days), or `all: true`"
        );
    }

    let lower_sec: Option<i64> = if b.all {
        None
    } else {
        b.from
            .as_ref()
            .map(|s| parse_day(s).map(|d| utc_day_start(d).timestamp()))
            .transpose()?
    };
    let upper_excl_sec: Option<i64> = if b.all {
        None
    } else {
        b.to.as_ref()
            .map(|s| parse_day(s).map(|d| day_after_exclusive(d).timestamp()))
            .transpose()?
    };

    if let (Some(lo), Some(hi)) = (lower_sec, upper_excl_sec) {
        if hi <= lo {
            anyhow::bail!("bundled_btcusd_1m: `to` day must be on or after `from` day");
        }
    }

    let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut lines = BufReader::new(f).lines();
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
        let mut parts = line.splitn(6, ',');
        let ts_s = parts.next().context("timestamp column")?;
        let o = parts.next().context("open")?;
        let h = parts.next().context("high")?;
        let l = parts.next().context("low")?;
        let c = parts.next().context("close")?;
        let v = parts.next().context("volume")?;

        let ts = row_ts_sec(ts_s)?;
        if let Some(lo) = lower_sec {
            if ts < lo {
                continue;
            }
        }
        if let Some(hi) = upper_excl_sec {
            if ts >= hi {
                break;
            }
        }

        let close_time = Utc
            .timestamp_opt(ts, 0)
            .single()
            .ok_or_else(|| anyhow!("bad unix timestamp {ts}"))?;
        out.push(Candle {
            close_time,
            open: o
                .trim()
                .parse()
                .with_context(|| format!("open line {}", lineno + 2))?,
            high: h.trim().parse().context("high")?,
            low: l.trim().parse().context("low")?,
            close: c.trim().parse().context("close")?,
            volume: v.trim().parse().context("volume")?,
            buy_volume: None,
            sell_volume: None,
            delta: None,
        });

        if out.len() > MAX_BUNDLED_1M_BARS {
            anyhow::bail!(
                "bundled BTC/USD 1m slice exceeds {MAX_BUNDLED_1M_BARS} rows; narrow `from`/`to` or increase cap in code"
            );
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
        let b = BundledBtcUsd1m {
            from: None,
            to: None,
            all: true,
        };
        let v = load_btcusd_1m_from_path(&path, &b).expect("load");
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn loads_fixture_day_slice() {
        let path = tiny_fixture();
        let b = BundledBtcUsd1m {
            from: Some("2000-01-01".to_string()),
            to: Some("2030-01-01".to_string()),
            all: false,
        };
        let v = load_btcusd_1m_from_path(&path, &b).expect("load");
        assert_eq!(v.len(), 4);
    }
}
