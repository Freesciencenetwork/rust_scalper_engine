# Manual Halt Flag Block

Scenario type: operator-imposed entry stop.

Market story:

- market structure is otherwise healthy
- no macro event blocks the trade
- the upstream orchestrator sends a manual halt flag

What we expect:

- the machine should refuse the setup
- the returned action should be `stand_aside`
- the decision reasons should include `daily_halt`
