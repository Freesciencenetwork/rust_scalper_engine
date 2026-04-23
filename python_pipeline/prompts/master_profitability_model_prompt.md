You are designing a new BTC profitability-filter model for the Rust scalper engine in this repository.

Current date: [[CURRENT_DATE]]
Requested model run name: [[MODEL_NAME]]
User brief: [[USER_BRIEF]]
Preferred thesis type: [[PREFERRED_THESIS_TYPE]]
Preferred base strategy: [[PREFERRED_BASE_STRATEGY]]
Target bar interval: [[BAR_INTERVAL]]
Feature count target: between [[MIN_FEATURE_COUNT]] and [[MAX_FEATURE_COUNT]] features total.

Your job:
1. Use web search to gather current, high-signal information on indicator behavior, trend / breakout / mean-reversion filters, and BTC or liquid-crypto intraday trading context relevant to this request.
2. Use the local repository constraints below to choose a trainable candidate model.
3. Return one strict JSON object only. No markdown, no prose outside JSON.

Hard constraints:
- You are not inventing a new executable Rust strategy. You must choose one base Rust strategy from the supported list below.
- You are designing a profitability gate on top of that base strategy.
- You must only use feature columns from the provided normalized feature inventory.
- Do not invent unsupported indicator names, unsupported strategy ids, or unsupported timeframes.
- Prefer layered feature sets with clear purpose, usually 4 to 7 layers.
- Prefer realistic indicator subsets, not giant kitchen-sink bundles.
- If a preferred thesis or preferred base strategy looks weak, say so in the JSON and choose a stronger supported option.
- Optimize for trainability and economic plausibility, not for storytelling.

What good output looks like:
- Chooses a concrete thesis type such as `trend_following`, `breakout`, `mean_reversion`, `continuation`, or `momentum_confirmation`.
- Picks one supported base Rust strategy that matches that thesis.
- Explains why the base strategy is a good substrate for a profitability gate.
- Uses current web research to justify indicator families or filters.
- Produces a clean feature-layer layout using only available columns.
- Recommends a sensible label buffer and training window.

Supported base Rust strategies:
[[BASE_STRATEGIES_JSON]]

Available normalized feature columns:
[[FEATURE_COLUMNS_JSON]]

Existing local strategy examples:
[[EXAMPLE_STRATEGIES_JSON]]

Output contract:
- Return exactly one JSON object with these keys:
  - `strategy_name`: short descriptive name
  - `version`: short version string like `v1`
  - `mode`: must be `profitability_filter`
  - `thesis_type`: short snake_case thesis label
  - `base_strategy_id`: one supported base strategy id
  - `timeframe`: use the requested interval unless there is a strong repo-grounded reason not to
  - `description`: 1 concise paragraph
  - `why_this_should_work`: array of 3 to 6 concrete claims
  - `online_research_takeaways`: array of 3 to 6 concise takeaways informed by web research
  - `risk_flags`: array of 2 to 6 caveats or failure modes
  - `label`: object with keys `type`, `buffer_r`, `description`
  - `training_window`: object with keys `from_date`, `to_date`, `warmup_days`, `n_folds`
  - `feature_layers`: object mapping layer names to arrays of feature column names
  - `layer_rationales`: object mapping each feature layer name to a short rationale
  - `alternatives_considered`: array of 2 or 3 objects, each with `base_strategy_id`, `thesis_type`, `reason_rejected`
  - `implementation_notes`: array of 2 to 6 practical notes for the training script

JSON quality rules:
- All dates must be absolute dates in `YYYY-MM-DD`.
- `label.type` must be `take_trade`.
- `label.buffer_r` must be numeric and usually between `0.1` and `0.5`.
- `training_window.n_folds` must be an integer between `3` and `6`.
- `training_window.warmup_days` must be an integer between `7` and `30`.
- `feature_layers` must have at least 3 layers.
- Every feature listed in `feature_layers` must appear in the provided feature inventory exactly as written.
- Keep total features between the requested min and max unless you explicitly note a strong reason in `implementation_notes`.

Decision standard:
- Be skeptical.
- Choose the candidate most likely to survive costs and slippage after filtering.
- Do not claim likely success unless the reasoning is tight.
- If the repo constraints make the requested idea weak, steer toward the strongest nearby viable candidate.

---

## After this JSON (human workflow — do not output this section as JSON)

Train and evaluate only through the **maintained** scripts in `python_pipeline/`:

1. **Features:** `ingest/prepare_feature_cache.py` (or the two-step ingest + `features/build_feature_cache.py` path documented in `python_pipeline/README.md`).
2. **Train end-to-end:** `workflows/run_profitability_workflow.py` with `--model-name`, `--strategy`, `--strategy-spec`, dates, and `--bar-interval` aligned to the JSON.
3. **Read test metrics:** Open `python_pipeline/models/runs/<slug>/README.md` — walk-forward fold tables (MCC, coverage, expectancy, profit factor) are the primary **model** validation. Rust engine tests: `cargo test --all-targets --locked` from repo root.

**Repo cleanliness:** Do not instruct adding new one-off or non-essential Python files under `python_pipeline/`. If a throwaway script is needed for an experiment, it should be **deleted after use** or its logic folded into `ingest/`, `features/`, `training/`, or `workflows/`. Same policy is stated in `python_pipeline/README.md` (root `README.md` documents the Rust HTTP server only).
