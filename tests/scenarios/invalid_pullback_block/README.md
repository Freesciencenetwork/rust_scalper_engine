# Invalid Pullback Block

Scenario type: bullish trend, but the entry candle does not retrace properly.

Market story:

- the broader trend and context remain constructive
- the final candle closes up, but its wick never actually tags the fast trend
  area deeply enough to qualify as a pullback entry

What we expect:

- the machine should refuse the setup
- the decision reasons should include `invalid_pullback`
