"""
prepare_rust_features.py — build the cached Rust-backed feature dataset used by training.

Usage
-----
  python3 prepare_rust_features.py
  python3 prepare_rust_features.py --server http://127.0.0.1:8080 --from 2020-01-01 --to 2024-12-31
"""

import argparse
import logging
import subprocess
import sys


logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)


def run_step(cmd: list[str]) -> None:
    logger.info("Running: %s", " ".join(cmd))
    subprocess.run(cmd, check=True)


def main():
    parser = argparse.ArgumentParser(description="Prepare Rust-backed feature cache for training")
    parser.add_argument("--server", default="http://localhost:8080")
    parser.add_argument("--from", dest="from_date", default="2012-01-02")
    parser.add_argument("--to", dest="to_date", default=None)
    parser.add_argument("--out", default="data/indicators_full.parquet")
    args = parser.parse_args()

    fetch_cmd = [
        sys.executable,
        "fetch_indicators.py",
        "--server", args.server,
        "--out", args.out,
        "--from", args.from_date,
    ]
    if args.to_date:
        fetch_cmd.extend(["--to", args.to_date])

    run_step(fetch_cmd)
    run_step([sys.executable, "normalize_features.py"])
    logger.info("Rust-backed feature cache is ready for train_v2.py")


if __name__ == "__main__":
    main()
