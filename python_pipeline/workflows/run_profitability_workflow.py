"""
run_profitability_workflow.py — End-to-end profitability model builder.

Workflow:
  1. Ensure the local Rust strategy server is running.
  2. Export a trade ledger from the Rust HTTP backtest endpoint.
  3. Train a profitability LightGBM model into one named folder.
  4. Write a markdown report with theory + results beside the artifacts.
"""

import argparse
import datetime as dt
import json
import logging
import math
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path


THIS_DIR = Path(__file__).resolve().parent
PACKAGE_ROOT = THIS_DIR.parent
REPO_ROOT = PACKAGE_ROOT.parent
DEFAULT_ENGINE_URL = os.environ.get("ENGINE_URL", "http://127.0.0.1:8080")
DEFAULT_RUNS_DIR = PACKAGE_ROOT / "models" / "runs"
DEFAULT_DATA_DIR = PACKAGE_ROOT / "data"

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-name", required=True)
    parser.add_argument("--strategy", required=True)
    parser.add_argument("--strategy-spec", required=True)
    parser.add_argument("--engine-url", default=DEFAULT_ENGINE_URL)
    parser.add_argument("--bar-interval", default="15m")
    parser.add_argument("--features", default=None)
    parser.add_argument("--from-date", default=None, help="UTC YYYY-MM-DD")
    parser.add_argument("--to-date", default=None, help="UTC YYYY-MM-DD")
    parser.add_argument("--warmup-days", type=int, default=14)
    parser.add_argument("--buffer-r", type=float, default=0.3)
    parser.add_argument("--n-folds", type=int, default=5)
    parser.add_argument("--entry-fee-bps", type=float, default=10.0)
    parser.add_argument("--exit-fee-bps", type=float, default=10.0)
    parser.add_argument("--entry-slippage-bps", type=float, default=2.0)
    parser.add_argument("--exit-slippage-bps", type=float, default=2.0)
    parser.add_argument("--stop-extra-slippage-bps", type=float, default=3.0)
    parser.add_argument("--max-hold-bars", type=int, default=20)
    parser.add_argument("--runs-dir", default=str(DEFAULT_RUNS_DIR))
    return parser.parse_args()


def slugify(text: str) -> str:
    slug = re.sub(r"[^a-zA-Z0-9._-]+", "_", text.strip()).strip("._-").lower()
    if not slug:
        raise SystemExit("model name produced an empty folder slug")
    return slug


def resolve_features_path(args) -> Path:
    if args.features:
        return Path(args.features).expanduser().resolve()

    candidates = [
        DEFAULT_DATA_DIR / f"features_normalized_{args.bar_interval}.parquet",
        DEFAULT_DATA_DIR / "features_normalized.parquet",
    ]
    for path in candidates:
        if path.exists():
            return path.resolve()
    raise SystemExit(
        "--features was not provided and no default normalized feature parquet was found"
    )


def check_health(engine_url: str) -> bool:
    url = engine_url.rstrip("/") + "/health"
    try:
        with urllib.request.urlopen(url, timeout=2) as resp:
            return resp.read().decode().strip().lower() == "ok"
    except (urllib.error.URLError, TimeoutError):
        return False


def ensure_server(engine_url: str, output_dir: Path):
    if check_health(engine_url):
        logger.info("Rust server already healthy at %s", engine_url)
        return None

    log_path = output_dir / "server.log"
    log_fh = open(log_path, "w")
    logger.info("Starting Rust server via cargo run --bin server")
    proc = subprocess.Popen(
        ["cargo", "run", "--bin", "server"],
        cwd=REPO_ROOT,
        stdout=log_fh,
        stderr=subprocess.STDOUT,
        text=True,
    )

    deadline = time.time() + 90
    while time.time() < deadline:
        if proc.poll() is not None:
            log_fh.close()
            tail = log_path.read_text(errors="replace")[-4000:] if log_path.exists() else ""
            raise SystemExit(f"Rust server exited early.\n\n{tail}")
        if check_health(engine_url):
            logger.info("Rust server is healthy at %s", engine_url)
            return {"process": proc, "log_file": log_fh, "log_path": str(log_path)}
        time.sleep(1)

    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=5)
    log_fh.close()
    raise SystemExit(f"Rust server did not become healthy within 90 seconds ({engine_url})")


def stop_server(server_state):
    if not server_state:
        return
    proc = server_state["process"]
    log_fh = server_state["log_file"]
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
    log_fh.close()


def run_command(cmd: list[str], cwd: Path):
    logger.info("Running: %s", " ".join(cmd))
    subprocess.run(cmd, cwd=cwd, check=True)


def render_feature_layers(strategy: dict) -> str:
    layers = strategy.get("feature_layers", {})
    if not layers:
        return "- None listed"
    lines = []
    for name, feats in layers.items():
        joined = ", ".join(feats)
        lines.append(f"- `{name}`: {joined}")
    return "\n".join(lines)


def fmt_float(value, digits=4) -> str:
    if value is None:
        return "n/a"
    try:
        value = float(value)
    except (TypeError, ValueError):
        return str(value)
    if math.isnan(value):
        return "n/a"
    return f"{value:.{digits}f}"


def render_fold_table(summary: dict) -> str:
    rows = [
        "| Fold | MCC | Coverage | Baseline Exp (R) | Filtered Exp (R) | Baseline PF | Filtered PF | Trades | Selected |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for idx, fold in enumerate(summary.get("fold_metrics", []), start=1):
        rows.append(
            "| {fold} | {mcc} | {cov} | {bexp} | {fexp} | {bpf} | {fpf} | {bcount} | {fcount} |".format(
                fold=idx,
                mcc=fmt_float(fold.get("mcc")),
                cov=fmt_float(100 * float(fold.get("coverage", 0.0))),
                bexp=fmt_float(fold.get("baseline_expectancy_r")),
                fexp=fmt_float(fold.get("filtered_expectancy_r")),
                bpf=fmt_float(fold.get("baseline_profit_factor")),
                fpf=fmt_float(fold.get("filtered_profit_factor")),
                bcount=fold.get("baseline_count", 0),
                fcount=fold.get("filtered_count", 0),
            )
        )
    return "\n".join(rows)


def write_report(
    output_dir: Path,
    model_name: str,
    args,
    strategy: dict,
    features_path: Path,
    schema: dict,
    ledger_summary: dict,
):
    summary = schema["summary"]
    now_utc = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    why = strategy.get("why_this_should_work", [])
    if why:
        why_block = "\n".join(f"- {line}" for line in why)
    else:
        why_block = "- Not provided in the strategy spec"

    description = strategy.get("description", "No description in strategy spec.")
    label = strategy.get("label") or strategy.get("label_definition", {})
    label_type = label.get("type", "take_trade")
    label_rule = label.get(
        "rule",
        f"net_r > {fmt_float(args.buffer_r, 2)}R",
    )
    markdown = f"""# {model_name}

## Theory

- Base strategy: `{args.strategy}`
- Strategy spec: `{Path(args.strategy_spec).name}`
- Mode: `{strategy.get("mode", "profitability_filter")}`
- Timeframe: `{strategy.get("timeframe", args.bar_interval)}`
- Label: `{label_type}` with rule `{label_rule}`
- Description: {description}

### Why This Should Work

{why_block}

### Feature Layers

{render_feature_layers(strategy)}

## Data Source

- Rust server: `{args.engine_url}`
- Backtest source: `POST /v1/strategies/{args.strategy}/backtest`
- Candle interval: `{args.bar_interval}`
- Normalized features parquet: `{features_path}`
- Date window: `{args.from_date or "all available"} -> {args.to_date or "all available"}`
- Warm-up days: `{args.warmup_days}`
- Costs: entry fee `{fmt_float(args.entry_fee_bps, 2)}` bps, exit fee `{fmt_float(args.exit_fee_bps, 2)}` bps, entry slippage `{fmt_float(args.entry_slippage_bps, 2)}` bps, exit slippage `{fmt_float(args.exit_slippage_bps, 2)}` bps
- Generated at: `{now_utc}`

## Results

- Ledger rows after feature join: `{ledger_summary.get("output_rows", "n/a")}`
- Raw resolved trades from backtest: `{ledger_summary.get("trade_rows", "n/a")}`
- Feature count used by trainer: `{summary.get("feature_count", "n/a")}`
- Positive label rate: `net_r > {fmt_float(schema.get("buffer_r"), 2)}R`
- Walk-forward MCC mean: `{fmt_float(summary.get("mcc_mean"))}`
- Walk-forward coverage mean: `{fmt_float(100 * float(summary.get("coverage_mean", 0.0)))}%`
- Baseline expectancy: `{fmt_float(summary.get("baseline_expectancy_r_mean"))}R`
- Filtered expectancy: `{fmt_float(summary.get("filtered_expectancy_r_mean"))}R`
- Baseline profit factor: `{fmt_float(summary.get("baseline_profit_factor_mean"))}`
- Filtered profit factor: `{fmt_float(summary.get("filtered_profit_factor_mean"))}`
- Baseline max drawdown: `{fmt_float(summary.get("baseline_max_drawdown_r_mean"))}R`
- Filtered max drawdown: `{fmt_float(summary.get("filtered_max_drawdown_r_mean"))}R`

### Fold Detail

{render_fold_table(summary)}

## Artifacts

- `strategy.json`
- `trade_ledger.parquet`
- `trade_ledger.summary.json`
- `profitability_lgbm.txt`
- `profitability_schema.json`
- `run_manifest.json`
"""
    (output_dir / "README.md").write_text(markdown)


def main():
    args = parse_args()
    model_slug = slugify(args.model_name)
    output_dir = Path(args.runs_dir).expanduser().resolve() / model_slug
    output_dir.mkdir(parents=True, exist_ok=True)

    features_path = resolve_features_path(args)
    strategy_spec_path = Path(args.strategy_spec).expanduser().resolve()
    if not strategy_spec_path.exists():
        raise SystemExit(f"strategy spec not found: {strategy_spec_path}")

    server_state = None
    try:
        server_state = ensure_server(args.engine_url, output_dir)

        ledger_path = output_dir / "trade_ledger.parquet"
        export_cmd = [
            sys.executable,
            str(PACKAGE_ROOT / "training" / "build_trade_ledger.py"),
            "--strategy",
            args.strategy,
            "--engine-url",
            args.engine_url,
            "--features",
            str(features_path),
            "--bar-interval",
            args.bar_interval,
            "--warmup-days",
            str(args.warmup_days),
            "--entry-fee-bps",
            str(args.entry_fee_bps),
            "--exit-fee-bps",
            str(args.exit_fee_bps),
            "--entry-slippage-bps",
            str(args.entry_slippage_bps),
            "--exit-slippage-bps",
            str(args.exit_slippage_bps),
            "--stop-extra-slippage-bps",
            str(args.stop_extra_slippage_bps),
            "--max-hold-bars",
            str(args.max_hold_bars),
            "--output",
            str(ledger_path),
        ]
        if args.from_date and args.to_date:
            export_cmd.extend(["--from-date", args.from_date, "--to-date", args.to_date])
        run_command(export_cmd, REPO_ROOT)

        train_cmd = [
            sys.executable,
            str(PACKAGE_ROOT / "training" / "train_profitability_filter.py"),
            "--strategy",
            args.strategy,
            "--data",
            str(ledger_path),
            "--strategy-spec",
            str(strategy_spec_path),
            "--buffer-r",
            str(args.buffer_r),
            "--n-folds",
            str(args.n_folds),
            "--output-dir",
            str(output_dir),
        ]
        run_command(train_cmd, REPO_ROOT)

        strategy = json.loads(strategy_spec_path.read_text())
        schema = json.loads((output_dir / "profitability_schema.json").read_text())
        ledger_summary = json.loads((output_dir / "trade_ledger.summary.json").read_text())

        manifest = {
            "model_name": args.model_name,
            "model_slug": model_slug,
            "output_dir": str(output_dir),
            "strategy": args.strategy,
            "strategy_spec": str(strategy_spec_path),
            "engine_url": args.engine_url,
            "bar_interval": args.bar_interval,
            "features": str(features_path),
            "from_date": args.from_date,
            "to_date": args.to_date,
            "buffer_r": args.buffer_r,
            "n_folds": args.n_folds,
            "server_started_by_script": bool(server_state),
            "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        }
        if server_state:
            manifest["server_log"] = server_state["log_path"]
        (output_dir / "run_manifest.json").write_text(json.dumps(manifest, indent=2))

        if strategy_spec_path.resolve() != (output_dir / "strategy.json").resolve():
            shutil.copyfile(strategy_spec_path, output_dir / "strategy.json")
        write_report(output_dir, args.model_name, args, strategy, features_path, schema, ledger_summary)
        logger.info("Model run saved to %s", output_dir)
    finally:
        stop_server(server_state)


if __name__ == "__main__":
    main()
