"""
prepare_feature_cache.py — build the cached Rust-backed feature dataset used by training.

Usage
-----
  python3 python_pipeline/ingest/prepare_feature_cache.py
  python3 python_pipeline/ingest/prepare_feature_cache.py --server http://127.0.0.1:8080 --from 2020-01-01 --to 2024-12-31
"""

import argparse
import logging
import subprocess
import sys
from pathlib import Path


logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)
ROOT_DIR = Path(__file__).resolve().parents[1]
INGEST_DIR = ROOT_DIR / "ingest"
FEATURES_DIR = ROOT_DIR / "features"
DATA_DIR = ROOT_DIR / "data"


def run_step(cmd: list[str]) -> None:
    logger.info("Running: %s", " ".join(cmd))
    subprocess.run(cmd, check=True)


def main():
    parser = argparse.ArgumentParser(description="Prepare Rust-backed feature cache for training")
    parser.add_argument("--server", default="http://localhost:8080")
    parser.add_argument("--from", dest="from_date", default="2012-01-02")
    parser.add_argument("--to", dest="to_date", default=None)
    parser.add_argument("--out", default=str(DATA_DIR / "indicators_full.parquet"))
    parser.add_argument("--features-out", default=str(DATA_DIR / "features_normalized.parquet"))
    parser.add_argument("--bar-interval", default="1m")
    parser.add_argument("--bundled-resample-interval", default=None)
    args = parser.parse_args()

    raw_metadata_out = f"{args.out.rsplit('.', 1)[0]}.metadata.json"
    features_metadata_out = f"{args.features_out.rsplit('.', 1)[0]}.metadata.json"
    fetch_cmd = [
        sys.executable,
        str(INGEST_DIR / "collect_indicators.py"),
        "--server", args.server,
        "--out", args.out,
        "--from", args.from_date,
        "--bar-interval", args.bar_interval,
        "--metadata-out", raw_metadata_out,
    ]
    if args.to_date:
        fetch_cmd.extend(["--to", args.to_date])
    if args.bundled_resample_interval:
        fetch_cmd.extend(["--bundled-resample-interval", args.bundled_resample_interval])

    run_step(fetch_cmd)
    run_step([
        sys.executable,
        str(FEATURES_DIR / "build_feature_cache.py"),
        "--in", args.out,
        "--out", args.features_out,
        "--in-metadata", raw_metadata_out,
        "--out-metadata", features_metadata_out,
    ])
    logger.info("Rust-backed feature cache is ready for profitability training")


if __name__ == "__main__":
    main()
