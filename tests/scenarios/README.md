# Scenario Fixtures

This folder contains market-scenario fixtures for the decision machine.

Each scenario lives in its own subfolder and contains:

- `README.md`
  Human explanation of the market structure being simulated.

- `machine_config.json`
  Strategy config used for that scenario. These fixtures use shorter lookbacks
  than production defaults so the examples stay compact and readable.

- `request.json`
  Mock normalized machine input.

- `expected.json`
  Expected high-level machine outcome.

The integration runner in `tests/scenario_suite.rs` loads every scenario folder
and executes it through the public `DecisionMachine` API.
