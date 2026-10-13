use monitor_contracts::*;

#[test]
fn query_is_explicit_and_rejects_unknown_fields() {
    let query: DashboardQuery = serde_json::from_str(r#"{"scenario":"negative"}"#).unwrap();
    assert_eq!(query.scenario, DemoScenario::Negative);
    assert!(serde_json::from_str::<DashboardQuery>(
        r#"{"scenario":"ready","credential":"forbidden"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<DashboardQuery>(r#"{"scenario":"unrecognized"}"#).is_err());
}

#[test]
fn integers_are_strings_and_missing_values_are_null() {
    let metric = Metric {
        value: Some("9007199254740993".to_string()),
        quality: QualityStatus::Known,
        reason_codes: Vec::new(),
    };
    let json = serde_json::to_value(&metric).unwrap();
    assert_eq!(json["value"], "9007199254740993");
    assert!(serde_json::from_value::<Metric<String>>(
        serde_json::json!({"value": 7,"quality":"known","reasonCodes":[]})
    )
    .is_err());
    let missing = Metric::<String> {
        value: None,
        quality: QualityStatus::Unknown,
        reason_codes: vec![ReasonCode::MissingComponent],
    };
    assert!(serde_json::to_value(missing).unwrap()["value"].is_null());
}

#[test]
fn synthetic_dashboard_round_trips_without_expanding_the_whitelist() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/demo.json")).unwrap();
    let dashboard: Dashboard = serde_json::from_value(fixtures["ready"].clone()).unwrap();
    assert_eq!(serde_json::to_value(dashboard).unwrap(), fixtures["ready"]);
    let mut invalid = fixtures["ready"].clone();
    invalid["rawResponse"] = "not allowed".into();
    assert!(serde_json::from_value::<Dashboard>(invalid).is_err());
}
