"""
data_loader.py — Loads and validates raw BTC OHLCV data from CSV.

Responsibility: only ingestion and sanity-checking.  No feature
engineering happens here — that belongs in features.py.
"""

import logging
import pandas as pd

logger = logging.getLogger(__name__)

REQUIRED_COLUMNS = {"timestamp", "open", "high", "low", "close", "volume"}


def load_ohlcv(path: str) -> pd.DataFrame:
    """
    Load a BTC OHLCV CSV, validate it, and return a clean DataFrame.

    Parameters
    ----------
    path : str
        Path to the CSV file.

    Returns
    -------
    pd.DataFrame
        Sorted by timestamp ascending, index reset, dtypes enforced.

    Raises
    ------
    FileNotFoundError
        If the CSV does not exist at *path*.
    ValueError
        If required columns are missing or the DataFrame is empty after
        cleaning.
    """
    logger.info("Loading OHLCV data from: %s", path)

    try:
        df = pd.read_csv(path)
    except FileNotFoundError:
        raise FileNotFoundError(f"CSV not found: {path}")

    # ---- column validation ------------------------------------------------
    missing = REQUIRED_COLUMNS - set(df.columns.str.lower())
    if missing:
        raise ValueError(f"CSV is missing required columns: {missing}")

    # Normalise column names to lowercase so the rest of the code is consistent
    df.columns = df.columns.str.lower()

    # ---- parse timestamp ---------------------------------------------------
    # Accept both unix-epoch integers and human-readable strings.
    # pd.to_datetime handles both; utc=False keeps it naive if no tz present.
    df["timestamp"] = pd.to_datetime(df["timestamp"], utc=False, infer_datetime_format=True)

    # ---- enforce numeric types --------------------------------------------
    for col in ("open", "high", "low", "close", "volume"):
        df[col] = pd.to_numeric(df[col], errors="coerce")

    # ---- drop obviously bad rows ------------------------------------------
    before = len(df)
    df.dropna(subset=["open", "high", "low", "close", "volume"], inplace=True)
    dropped = before - len(df)
    if dropped:
        logger.warning("Dropped %d rows with NaN in OHLCV columns.", dropped)

    # ---- sort chronologically & reset index --------------------------------
    df.sort_values("timestamp", inplace=True)
    df.reset_index(drop=True, inplace=True)

    if df.empty:
        raise ValueError("DataFrame is empty after loading and cleaning.")

    logger.info(
        "Loaded %d candles from %s to %s",
        len(df),
        df["timestamp"].iloc[0],
        df["timestamp"].iloc[-1],
    )
    return df
