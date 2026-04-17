use std::fs;
use std::path::{Path, PathBuf};

use btc_continuation_v1::{DecisionMachine, MachineAction, MachineRequest, StrategyConfig};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ScenarioExpected {
    action: String,
    allowed: bool,
    plan_present: bool,
    required_reasons: Vec<String>,
}

#[test]
fn scenario_fixtures_execute_through_public_machine_api() {
    let scenarios_dir = Path::new("tests/scenarios");
    let mut scenario_dirs: Vec<PathBuf> = fs::read_dir(scenarios_dir)
        .expect("read scenarios dir")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.is_dir() { Some(path) } else { None }
        })
        .collect();
    scenario_dirs.sort();

    assert!(
        !scenario_dirs.is_empty(),
        "expected at least one scenario fixture"
    );

    for scenario_dir in scenario_dirs {
        run_scenario(&scenario_dir);
    }
}

fn run_scenario(scenario_dir: &Path) {
    let config: StrategyConfig = read_json(&scenario_dir.join("machine_config.json"));
    let request: MachineRequest = read_json(&scenario_dir.join("request.json"));
    let expected: ScenarioExpected = read_json(&scenario_dir.join("expected.json"));

    let machine = DecisionMachine::new(config);
    let response = machine.evaluate(request).unwrap_or_else(|error| {
        panic!(
            "scenario '{}' failed to evaluate: {error}",
            scenario_name(scenario_dir)
        )
    });

    assert_eq!(
        action_label(&response.action),
        expected.action,
        "scenario '{}' action mismatch; reasons: {:?}",
        scenario_name(scenario_dir),
        response.decision.reasons
    );
    assert_eq!(
        response.decision.allowed,
        expected.allowed,
        "scenario '{}' allowed mismatch; reasons: {:?}",
        scenario_name(scenario_dir),
        response.decision.reasons
    );
    assert_eq!(
        response.plan.is_some(),
        expected.plan_present,
        "scenario '{}' plan presence mismatch",
        scenario_name(scenario_dir)
    );

    for required_reason in expected.required_reasons {
        assert!(
            response
                .decision
                .reasons
                .iter()
                .any(|reason| reason == &required_reason),
            "scenario '{}' missing required reason '{}'",
            scenario_name(scenario_dir),
            required_reason
        );
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let body = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read '{}': {error}", path.display()));
    serde_json::from_str(&body)
        .unwrap_or_else(|error| panic!("failed to parse '{}': {error}", path.display()))
}

fn scenario_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn action_label(action: &MachineAction) -> String {
    match action {
        MachineAction::StandAside => "stand_aside".to_string(),
        MachineAction::ArmLongStop => "arm_long_stop".to_string(),
    }
}
