# Daily Halt Block

Scenario type: healthy market structure, but no new entries allowed.

Market story:

- the chart still looks constructive
- there is no nearby macro event
- the machine receives runtime state showing the daily `R` loss limit is already
  breached

What we expect:

- the machine should refuse the setup even though price structure is otherwise
  usable
- the returned action should be `stand_aside`
- the decision reasons should include `daily_halt`
- no plan should be emitted
