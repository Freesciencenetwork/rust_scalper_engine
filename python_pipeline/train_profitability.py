"""
train_profitability.py — Walk-forward profitability filter on exported trade ledgers.

Label:
  take_trade = 1 if net_r > PROFITABILITY_BUFFER_R else 0

Primary trading metrics:
  expectancy, profit factor, coverage, max drawdown
"""

import argparse
import datetime as dt
import json
import logging
import os
import sys

import numpy as np
import pandas as pd
from lightgbm import LGBMClassifier, early_stopping, log_evaluation
from sklearn.metrics import matthews_corrcoef

import config
from walk_forward import expanding_window_splits, slice_fold

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
    stream=sys.stdout,
)
logger = logging.getLogger(__name__)

EXCLUDED_COLUMNS = {
    "strategy_id",
    "signal_bar_index",
    "entry_bar_index",
    "exit_bar_index",
    "signal_close_time_ms",
    "entry_close_time",
    "exit_close_time",
    "entry_price_raw",
    "entry_price_fill",
    "exit_price_raw",
    "exit_price_fill",
    "trigger_price",
    "stop_price",
    "target_price",
    "atr_at_signal",
    "bars_held",
    "exit_reason",
    "gross_return_pct",
    "gross_r",
    "fee_cost_pct",
    "slippage_cost_pct",
    "net_return_pct",
    "net_r",
    "profitable",
    "entry_fee_bps",
    "exit_fee_bps",
    "entry_slippage_bps",
    "exit_slippage_bps",
    "stop_extra_slippage_bps",
    "max_hold_bars",
    "timestamp_ms",
    "take_trade",
}


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--strategy", default="default")
    parser.add_argument("--data", default=None)
    parser.add_argument("--n-folds", type=int, default=config.WF_N_FOLDS)
    parser.add_argument("--buffer-r", type=float, default=config.PROFITABILITY_BUFFER_R)
    return parser.parse_args()


def fit_lgbm_binary(X_train, y_train, X_val, y_val):
    params = dict(config.LGBM_PROFITABILITY_PARAMS)
    model = LGBMClassifier(**params)
    model.fit(
        X_train,
        y_train,
        eval_set=[(X_val, y_val)],
        callbacks=[
            early_stopping(config.LGBM_BINARY_EARLY_STOPPING, verbose=False),
            log_evaluation(period=-1),
        ],
    )
    return model


def feature_columns(df: pd.DataFrame) -> list[str]:
    cols = []
    for col in df.columns:
        if col in EXCLUDED_COLUMNS:
            continue
        if pd.api.types.is_numeric_dtype(df[col]):
            cols.append(col)
    return cols


def profit_factor(net_r: np.ndarray) -> float:
    wins = net_r[net_r > 0].sum()
    losses = net_r[net_r < 0].sum()
    if losses >= 0:
        return np.nan
    return float(wins / abs(losses))


def max_drawdown_r(net_r: np.ndarray) -> float:
    if len(net_r) == 0:
        return 0.0
    equity = np.cumsum(net_r)
    peak = np.maximum.accumulate(np.maximum(equity, 0.0))
    drawdown = peak - equity
    return float(drawdown.max()) if len(drawdown) else 0.0


def trade_metrics(net_r: np.ndarray) -> dict:
    if len(net_r) == 0:
        return {
            "count": 0,
            "expectancy_r": np.nan,
            "profit_factor": np.nan,
            "max_drawdown_r": 0.0,
        }
    return {
        "count": int(len(net_r)),
        "expectancy_r": float(net_r.mean()),
        "profit_factor": profit_factor(net_r),
        "max_drawdown_r": max_drawdown_r(net_r),
    }


def evaluate_fold(y_true: np.ndarray, y_pred: np.ndarray, net_r: np.ndarray) -> dict:
    selected = y_pred == 1
    selected_net_r = net_r[selected]
    base = trade_metrics(net_r)
    filt = trade_metrics(selected_net_r)
    coverage = float(selected.mean()) if len(selected) else np.nan
    mcc = float(matthews_corrcoef(y_true, y_pred)) if len(np.unique(y_pred)) > 1 else 0.0
    return {
        "mcc": mcc,
        "coverage": coverage,
        "baseline_expectancy_r": base["expectancy_r"],
        "filtered_expectancy_r": filt["expectancy_r"],
        "baseline_profit_factor": base["profit_factor"],
        "filtered_profit_factor": filt["profit_factor"],
        "baseline_max_drawdown_r": base["max_drawdown_r"],
        "filtered_max_drawdown_r": filt["max_drawdown_r"],
        "baseline_count": base["count"],
        "filtered_count": filt["count"],
    }


def main():
    args = parse_args()
    data_path = args.data or os.path.join(
        config.DATA_DIR, f"trade_ledger_{args.strategy}.parquet"
    )
    logger.info("Loading trade ledger from %s", data_path)
    df = pd.read_parquet(data_path).sort_values("signal_close_time_ms").reset_index(drop=True)
    df["take_trade"] = (df["net_r"] > args.buffer_r).astype(int)

    feat_cols = feature_columns(df)
    logger.info(
        "Rows=%d  Features=%d  Positive labels=%d (%.1f%%)  Buffer=%.2fR",
        len(df),
        len(feat_cols),
        int(df["take_trade"].sum()),
        100 * df["take_trade"].mean(),
        args.buffer_r,
    )

    splits = expanding_window_splits(len(df), n_folds=args.n_folds, val_ratio=config.WF_VAL_RATIO)
    fold_metrics = []

    for split in splits:
        train_df, val_df, test_df = slice_fold(df, split)
        X_tr = train_df[feat_cols]
        y_tr = train_df["take_trade"].values.astype(int)
        X_vl = val_df[feat_cols]
        y_vl = val_df["take_trade"].values.astype(int)
        X_te = test_df[feat_cols]
        y_te = test_df["take_trade"].values.astype(int)
        net_r = test_df["net_r"].values.astype(float)

        model = fit_lgbm_binary(X_tr, y_tr, X_vl, y_vl)
        proba = model.predict_proba(X_te)[:, 1]
        pred = (proba >= 0.5).astype(int)
        metrics = evaluate_fold(y_te, pred, net_r)
        fold_metrics.append(metrics)
        logger.info(
            "Fold %d | MCC=%+.4f  coverage=%.1f%%  filtered_exp=%.4fR  baseline_exp=%.4fR",
            split["fold"],
            metrics["mcc"],
            100 * metrics["coverage"],
            metrics["filtered_expectancy_r"],
            metrics["baseline_expectancy_r"],
        )

    if not fold_metrics:
        raise SystemExit("no folds completed")

    def mean_metric(name: str) -> float:
        vals = [m[name] for m in fold_metrics if not np.isnan(m[name])]
        return float(np.mean(vals)) if vals else float("nan")

    summary = {
        "strategy": args.strategy,
        "buffer_r": args.buffer_r,
        "rows": len(df),
        "feature_count": len(feat_cols),
        "mcc_mean": mean_metric("mcc"),
        "coverage_mean": mean_metric("coverage"),
        "baseline_expectancy_r_mean": mean_metric("baseline_expectancy_r"),
        "filtered_expectancy_r_mean": mean_metric("filtered_expectancy_r"),
        "baseline_profit_factor_mean": mean_metric("baseline_profit_factor"),
        "filtered_profit_factor_mean": mean_metric("filtered_profit_factor"),
        "baseline_max_drawdown_r_mean": mean_metric("baseline_max_drawdown_r"),
        "filtered_max_drawdown_r_mean": mean_metric("filtered_max_drawdown_r"),
        "fold_metrics": fold_metrics,
    }

    logger.info(
        "Summary | MCC=%+.4f  coverage=%.1f%%  filtered_exp=%.4fR  baseline_exp=%.4fR",
        summary["mcc_mean"],
        100 * summary["coverage_mean"],
        summary["filtered_expectancy_r_mean"],
        summary["baseline_expectancy_r_mean"],
    )

    full_train = df.iloc[: splits[-1]["test"][0]].reset_index(drop=True)
    val_size = max(1, int(len(full_train) * config.WF_VAL_RATIO))
    X_full = full_train[feat_cols]
    y_full = full_train["take_trade"].values.astype(int)
    final_model = fit_lgbm_binary(
        X_full[:-val_size], y_full[:-val_size], X_full[-val_size:], y_full[-val_size:]
    )

    date_tag = dt.datetime.utcnow().strftime("%Y%m%d")
    metric_tag = f"exp{summary['filtered_expectancy_r_mean']:+.3f}".replace("+", "p").replace(
        "-", "m"
    )
    ckpt_dir = os.path.join(
        config.MODELS_DIR,
        "checkpoints",
        f"btc_1m_profitability_{args.strategy}_lgbm_{metric_tag}_{date_tag}",
    )
    os.makedirs(ckpt_dir, exist_ok=True)

    model_path = os.path.join(ckpt_dir, "profitability_lgbm.txt")
    final_model.booster_.save_model(model_path)
    with open(os.path.join(ckpt_dir, "profitability_schema.json"), "w") as fh:
        json.dump(
            {
                "strategy": args.strategy,
                "buffer_r": args.buffer_r,
                "feature_columns": feat_cols,
                "summary": summary,
            },
            fh,
            indent=2,
        )
    logger.info("Saved checkpoint -> %s", ckpt_dir)


if __name__ == "__main__":
    main()
