use es_backend::registry::{force_var, select, select_all};
use es_backend::{BackendError, Env, Probe, Provider, ProviderReport, VirtualInput};
use es_geom::Point;

struct Fake;

impl VirtualInput for Fake {
    fn scroll(&self, _at: Point, _dy: f64) -> es_backend::Result<()> {
        Ok(())
    }
}

const LOW: Provider<dyn VirtualInput> = Provider {
    name: "low",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(Fake)),
};
const MID: Provider<dyn VirtualInput> = Provider {
    name: "mid",
    probe: |_| Some(5),
    build: |_| Ok(Box::new(Fake)),
};
const HIGH: Provider<dyn VirtualInput> = Provider {
    name: "high",
    probe: |_| Some(9),
    build: |_| Ok(Box::new(Fake)),
};
const BROKEN: Provider<dyn VirtualInput> = Provider {
    name: "broken",
    probe: |_| Some(200),
    build: |_| Err(BackendError::Unavailable("socket refused".into())),
};
const ABSENT: Provider<dyn VirtualInput> = Provider {
    name: "absent",
    probe: |_| None,
    build: |_| panic!("an undetected provider must never be built"),
};

fn pick(list: &[Provider<dyn VirtualInput>], probe: &Probe) -> ProviderReport {
    let mut report = ProviderReport::default();
    select("thing", list, probe, &mut report);
    report
}

fn reason<'a>(report: &'a ProviderReport, provider: &str) -> &'a str {
    report.entries[0]
        .rejected
        .iter()
        .find(|(p, _)| *p == provider)
        .map(|(_, r)| r.as_str())
        .unwrap_or_else(|| panic!("{provider} not recorded as rejected"))
}

#[test]
fn highest_priority_wins_whatever_its_place_in_the_list() {
    let report = pick(&[LOW, HIGH, MID], &Probe::default());
    assert_eq!(report.chosen("thing"), Some("high"));
    assert_eq!(reason(&report, "mid"), "outranked by high");
    assert_eq!(reason(&report, "low"), "outranked by high");
}

#[test]
fn list_order_breaks_a_priority_tie() {
    const MID_TOO: Provider<dyn VirtualInput> = Provider {
        name: "mid-too",
        ..MID
    };
    let report = pick(&[MID_TOO, MID], &Probe::default());
    assert_eq!(report.chosen("thing"), Some("mid-too"));
}

#[test]
fn failing_build_falls_through_with_its_reason() {
    let report = pick(&[BROKEN, MID], &Probe::default());
    assert_eq!(report.chosen("thing"), Some("mid"));
    assert!(reason(&report, "broken").contains("socket refused"));
}

#[test]
fn undetected_provider_is_recorded_and_never_built() {
    let report = pick(&[ABSENT, LOW], &Probe::default());
    assert_eq!(report.chosen("thing"), Some("low"));
    assert_eq!(reason(&report, "absent"), "not detected");
}

#[test]
fn nothing_usable_chooses_nothing() {
    let mut report = ProviderReport::default();
    assert!(select("thing", &[ABSENT, BROKEN], &Probe::default(), &mut report).is_none());
    assert_eq!(report.chosen("thing"), None);
    assert_eq!(report.entries[0].rejected.len(), 2);
}

fn forced(value: &str) -> Probe {
    Probe {
        env: Env::from_vars([(force_var("thing"), value.to_owned())]),
        ..Probe::default()
    }
}

#[test]
fn force_variable_picks_a_lower_provider() {
    assert_eq!(force_var("thing"), "EASYSHOT_PROVIDER_THING");
    let report = pick(&[HIGH, LOW], &forced("low"));
    assert_eq!(report.chosen("thing"), Some("low"));
    assert_eq!(report.entries[0].forced.as_deref(), Some("low"));
    assert!(reason(&report, "high").contains("EASYSHOT_PROVIDER_THING"));
}

#[test]
fn forcing_an_unknown_provider_chooses_nothing_and_says_so() {
    let report = pick(&[HIGH, LOW], &forced("nonesuch"));
    assert_eq!(report.chosen("thing"), None);
    assert_eq!(report.entries[0].forced.as_deref(), Some("nonesuch"));
}

#[test]
fn select_all_keeps_every_usable_provider_best_first() {
    let mut report = ProviderReport::default();
    let all = select_all(
        "thing",
        &[LOW, BROKEN, HIGH, ABSENT],
        &Probe::default(),
        &mut report,
    );
    assert_eq!(all.len(), 2);
    assert_eq!(report.chosen("thing"), Some("high"));
    assert!(reason(&report, "broken").contains("socket refused"));
}
