# binance_spot_candles

Small Rust library for **read-only** Binance Spot market data:

- `GET /api/v3/klines` → normalized `Candle` rows
- `GET /api/v3/exchangeInfo` → `SymbolFilters` (`tick_size`, `lot_step`)
- optional CSV kline parsing (Binance export format)

Binary: `cargo run --bin binance-fetch` (same CLI as before in the parent repo).

This crate is the piece intended for **`cargo publish`** to crates.io. The parent workspace crate (`binance_BTC`) is **not** published (`publish = false`).
