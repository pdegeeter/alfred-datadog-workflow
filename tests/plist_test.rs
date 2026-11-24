//! Tests for the Alfred graph in `workflow/info.plist`: keywords, the
//! subcommand each one runs, the modifier connections to the action and the
//! user configuration.
//!
//! The plist is converted to JSON with macOS `plutil` (CI runs on macOS).

use std::collections::BTreeMap;
use std::process::Command;

use serde_json::Value;

/// Alfred's bitmask for each modifier the script filters emit.
const MODIFIERS: [(u64, &str); 6] = [
    (0, "none"),
    (1_048_576, "cmd"),
    (524_288, "alt"),
    (262_144, "ctrl"),
    (131_072, "shift"),
    (1_179_648, "cmd+shift"),
];

/// Every keyword and the command line it runs.
const KEYWORDS: [(&str, &str); 0] = [];

fn plist() -> Value {
    let output = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-", "workflow/info.plist"])
        .output()
        .expect("plutil should run");

    assert!(output.status.success(), "plutil failed: {output:?}");

    serde_json::from_slice(&output.stdout).expect("plist should convert to JSON")
}

fn script_filters(plist: &Value) -> Vec<&Value> {
    plist["objects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|object| object["type"] == "alfred.workflow.input.scriptfilter")
        .collect()
}

#[test]
fn every_keyword_runs_its_subcommand() {
    let plist = plist();
    let keywords: BTreeMap<String, String> = script_filters(&plist)
        .into_iter()
        .map(|filter| {
            (
                filter["config"]["keyword"].as_str().unwrap().to_owned(),
                filter["config"]["script"].as_str().unwrap().to_owned(),
            )
        })
        .collect();

    let expected: BTreeMap<String, String> = KEYWORDS
        .into_iter()
        .map(|(keyword, script)| (keyword.to_owned(), script.to_owned()))
        .collect();

    assert_eq!(keywords, expected);
}

#[test]
fn script_filters_leave_filtering_and_ordering_to_the_binary() {
    let plist = plist();

    for filter in script_filters(&plist) {
        assert_eq!(filter["config"]["alfredfiltersresults"], false);
    }
}

#[test]
fn every_script_filter_reaches_the_action_with_every_modifier() {
    let plist = plist();
    let action_uid = plist["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["type"] == "alfred.workflow.action.script")
        .map(|object| object["uid"].as_str().unwrap().to_owned())
        .expect("a Run Script action");

    for filter in script_filters(&plist) {
        let uid = filter["uid"].as_str().unwrap();
        let connections = plist["connections"][uid].as_array().unwrap();

        for (mask, name) in MODIFIERS {
            assert!(
                connections.iter().any(|connection| {
                    connection["destinationuid"] == action_uid.as_str()
                        && connection["modifiers"] == mask
                }),
                "{} has no `{name}` connection to the action",
                filter["config"]["keyword"]
            );
        }
    }
}

#[test]
fn the_action_runs_the_binary_and_feeds_a_notification() {
    let plist = plist();
    let objects = plist["objects"].as_array().unwrap();
    let action = objects
        .iter()
        .find(|object| object["type"] == "alfred.workflow.action.script")
        .unwrap();
    let notification = objects
        .iter()
        .find(|object| object["type"] == "alfred.workflow.output.notification")
        .unwrap();

    assert_eq!(action["config"]["script"], "./alfred-dd run \"$1\"");
    assert_eq!(
        plist["connections"][action["uid"].as_str().unwrap()][0]["destinationuid"],
        notification["uid"]
    );
}

#[test]
fn workflow_readme_lists_every_keyword() {
    let plist = plist();
    let readme = plist["readme"].as_str().unwrap();

    for filter in script_filters(&plist) {
        let keyword = filter["config"]["keyword"].as_str().unwrap();

        assert!(
            readme.contains(&format!("- {keyword} ")),
            "workflow readme does not document `{keyword}`"
        );
    }
}

#[test]
fn site_and_env_are_configurable_with_their_defaults() {
    let plist = plist();
    let defaults: BTreeMap<String, String> = plist["userconfigurationconfig"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| {
            (
                field["variable"].as_str().unwrap().to_owned(),
                field["config"]["default"].as_str().unwrap().to_owned(),
            )
        })
        .collect();

    assert_eq!(
        defaults,
        BTreeMap::from([
            ("DD_ENV".to_owned(), "production".to_owned()),
            ("DD_SITE".to_owned(), "datadoghq.eu".to_owned()),
        ])
    );
}
