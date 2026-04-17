# Bullish Continuation Entry

Scenario type: clean bullish continuation.

Market story:

- the market has been trending up for long enough to establish both `15m` and
  `1h` bullish trend structure
- flow remains positive
- the last candle pulls back into the fast trend area and closes back strong
- there is no macro event nearby
- there is no daily halt in force

What we expect:

- the machine should allow the setup
- the returned action should be `arm_long_stop`
- the machine should produce a plan with trigger, stop, target, and size hints
