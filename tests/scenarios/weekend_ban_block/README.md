# Weekend Ban Block

Scenario type: structurally valid setup blocked by time policy.

Market story:

- trend, flow, and pullback quality are acceptable
- there is no macro event nearby
- the final evaluation bar is moved into the weekend-ban window

What we expect:

- the machine should refuse the setup
- the returned action should be `stand_aside`
- the decision reasons should include `weekend_ban`
