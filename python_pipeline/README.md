# Python Pipeline

This file is the **README for `python_pipeline/`**: layout, training, testing, and hygiene. The repo root [`README.md`](../README.md) documents the **Rust HTTP server** only; use this file for the Python profitability workflow.

Run the shell examples below from the **repository root** (the directory that contains `python_pipeline/`), unless you adjust paths.

This folder is centered on the Rust-backed profitability workflow.

## Source of truth: Rust HTTP API

- **Indicators** — All feature columns ultimately come from **indicator replay responses** produced by the running **`server` binary** (`POST /v1/indicators/.../replay` and related routes). Ingest scripts (`ingest/collect_indicators.py`, `ingest/prepare_feature_cache.py`) are HTTP clients; they do not recompute indicators in Python.
- **OHLCV** — For the default BTC bundle, the server loads **`src/historical_data/`** (see root `README.md`). Pipeline commands pass **`bundled_btcusd_1m`** date ranges (or rely on the server defaults) so candles and resampling stay **inside the engine** the same way as manual `curl` tests.

## Layout

- `ingest/collect_indicators.py`
  Pulls indicator replays from the Rust HTTP API.
- `ingest/prepare_feature_cache.py`
  Builds the raw indicator cache and normalized feature cache in one step.
- `features/build_feature_cache.py`
  Converts raw indicator dumps into the normalized feature parquet used for training.
- `training/build_trade_ledger.py`
  Calls the Rust backtest endpoint and joins the resulting trade ledger to normalized features.
- `training/train_profitability_filter.py`
  Trains the LightGBM profitability filter from an exported ledger.
- `workflows/run_profitability_workflow.py`
  Runs the end-to-end local workflow: ensure server, export ledger, train, write run folder results.
- `shared/pipeline_config.py`
  Shared training constants.
- `shared/walk_forward_splits.py`
  Shared walk-forward split logic.
- `strategies/`
  Saved strategy specs used by the profitability trainer.
- `prompts/`
  Master AI prompt templates.
- `models/runs/`
  One folder per model run, including strategy, research, ledger, model, and markdown results. See [`models/runs/README.md`](models/runs/README.md) for a cross-run status index (what has been tried, what works, what failed), and [`strategies/strategy_overview.md`](strategies/strategy_overview.md) for the consolidated “what went good / what went wrong” retrospective.
- `data/`
  Cached parquet inputs.

## Steps to train a profitability model

1. **Install Python dependencies** (once):

   ```bash
   pip install -r python_pipeline/requirements.txt
   ```

2. **Rust HTTP server** — must be reachable for step 3 (`prepare_feature_cache.py` calls the API). Start manually (`cargo run` / `cargo run --bin server` from repo root) or let step 4 start it: `run_profitability_workflow.py` spawns the server if `/health` is not OK.

3. **Feature cache** — indicator replays + normalized parquet used when joining the trade ledger:

   ```bash
   python3 python_pipeline/ingest/prepare_feature_cache.py \
     --server http://127.0.0.1:8080 \
     --from 2020-01-01 \
     --bar-interval 15m \
     --bundled-resample-interval 15m \
     --out python_pipeline/data/indicators_full_15m.parquet \
     --features-out python_pipeline/data/features_normalized_15m.parquet
   ```

4. **Train** — ledger export (backtest API) + LightGBM + run folder (tune `--model-name`, `--strategy`, `--strategy-spec`, dates, `--bar-interval`):

   ```bash
   python3 python_pipeline/workflows/run_profitability_workflow.py \
     --model-name supertrend_adx_profit_gate_15m_v1 \
     --strategy supertrend_adx \
     --strategy-spec python_pipeline/strategies/supertrend_adx_profit_gate_15m_v1.json \
     --bar-interval 15m \
     --from-date 2022-01-01 \
     --to-date 2024-12-31
   ```

5. **Results** — `models/runs/<slug>/README.md` (metrics + fold table), plus `trade_ledger.*`, `profitability_lgbm.txt`, `profitability_schema.json`, etc. in that folder.

## Testing a trained model

There is no separate “inference-only” entrypoint required for basic validation:

- **`train_profitability_filter.py`** evaluates each **walk-forward test fold** (held-out time ranges) and logs fold metrics (e.g. MCC, coverage, baseline vs filtered expectancy and profit factor).
- **`run_profitability_workflow.py`** runs that trainer after building the ledger and writes a human-readable **`README.md`** plus `trade_ledger.*`, `profitability_lgbm.txt`, `profitability_schema.json`, and related artifacts under `models/runs/<slug>/`.

Open `python_pipeline/models/runs/<your_run>/README.md` for the summary table and fold breakdown. To re-train on an existing ledger you built yourself:

```bash
python3 python_pipeline/training/train_profitability_filter.py \
  --data path/to/ledger.parquet \
  --strategy-spec python_pipeline/strategies/your_spec.json \
  --output-dir python_pipeline/models/runs/your_run_name
```

Rust-side regression: from repo root, `cargo test --all-targets --locked`.

## Repository hygiene (Python)

- Keep **`python_pipeline/`** limited to **durable** stages: ingest, features, training, workflows, shared config, prompts, and checked-in strategy JSON under `strategies/`.
- **Do not commit** one-off scrapers, notebooks, or scratch scripts that are not part of that pipeline. If you create something temporary for debugging, **remove it** when done (or merge the useful parts into an existing script above).
- Prefer **extending** an existing script with flags over adding new top-level `.py` files.

Design notes for new profitability specs (AI-assisted): see `prompts/master_profitability_model_prompt.md`.
