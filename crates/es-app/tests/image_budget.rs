use es_app::image_budget::{ImageBudget, Lru};

#[test]
fn the_least_recently_used_goes_first() {
    let mut lru = Lru::new(250);
    assert!(lru.insert("a", 100).is_empty());
    assert!(lru.insert("b", 100).is_empty());
    assert_eq!(lru.insert("c", 100), ["a"]);
    assert_eq!(lru.used(), 200);
}

#[test]
fn touching_protects_an_entry_from_eviction() {
    let mut lru = Lru::new(250);
    lru.insert("a", 100);
    lru.insert("b", 100);
    lru.touch("a");
    assert_eq!(lru.insert("c", 100), ["b"]);
    assert!(lru.contains("a"));
}

#[test]
fn one_large_insert_evicts_in_age_order_and_keeps_itself() {
    let mut lru = Lru::new(250);
    lru.insert("a", 100);
    lru.insert("b", 100);
    assert_eq!(lru.insert("huge", 400), ["a", "b"]);
    assert!(lru.contains("huge"));
    assert_eq!(lru.used(), 400);
}

#[test]
fn replacing_a_key_counts_its_bytes_once() {
    let mut lru = Lru::new(250);
    lru.insert("a", 100);
    assert!(lru.insert("a", 150).is_empty());
    assert_eq!(lru.used(), 150);
    assert!(lru.remove("a"));
    assert!(!lru.remove("a"));
    assert_eq!(lru.used(), 0);
}

#[test]
fn the_default_budget_is_256_mebibytes() {
    let budget = ImageBudget::default();
    let mib = 1024 * 1024;
    assert!(budget.insert("a", 200 * mib).is_empty());
    assert_eq!(budget.insert("b", 100 * mib), ["a"]);
    assert_eq!(budget.used(), 100 * mib);
}
