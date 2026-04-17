# Macro Veto Block

Scenario type: bullish setup blocked by scheduled macro risk.

Market story:

- price structure is otherwise healthy and trending
- the setup would normally qualify as a continuation entry
- a high-impact macro event is close enough to the latest candle to activate the
  hard veto window

What we expect:

- the machine should refuse the setup
- the returned action should be `stand_aside`
- the decision reasons should include `macro_veto`
- no position plan should be emitted
