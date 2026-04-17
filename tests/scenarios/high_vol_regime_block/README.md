# High-Vol Regime Block

Scenario type: setup blocked because the latest bar moves too violently.

Market story:

- the broader trend is constructive
- the final bar expands far beyond the recent volatility baseline
- the machine should classify the environment as high-vol and refuse entry

What we expect:

- the machine should return `stand_aside`
- the decision reasons should include `high_vol_regime`
