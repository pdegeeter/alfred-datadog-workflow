//! Tests for name matching and ordering.

use alfred_dd::ranking::{normalize, rank, terms, tier, Tier};

fn query(text: &str) -> Vec<String> {
    terms(&text.split_whitespace().collect::<Vec<_>>())
}

fn ranked(names: &[&str], text: &str) -> Vec<String> {
    rank(
        names.iter().map(|name| name.to_string()).collect(),
        &query(text),
        |name| name.as_str(),
        |_| 0.0,
        |a, b| a.cmp(b),
    )
}

#[test]
fn normalize_lowercases_and_strips_accents() {
    assert_eq!(
        normalize("Équipe Paiements · Latence"),
        "equipe paiements · latence"
    );
    assert_eq!(
        normalize("Ça déçoit ÖSTERREICH œuvre"),
        "ca decoit osterreich oeuvre"
    );
}

#[test]
fn accents_match_both_ways() {
    assert_eq!(
        tier("Équipe Paiements", &query("equipe")),
        Some(Tier::Prefix)
    );
    assert_eq!(
        tier("Equipe Paiements", &query("équipe")),
        Some(Tier::Prefix)
    );
}

#[test]
fn tiers() {
    assert_eq!(
        tier("Checkout API", &query("checkout api")),
        Some(Tier::Exact)
    );
    assert_eq!(
        tier("Checkout API overview", &query("checkout")),
        Some(Tier::Prefix)
    );
    assert_eq!(
        tier("Payments checkout-api", &query("checkout")),
        Some(Tier::WordPrefix)
    );
    assert_eq!(
        tier("Payments api overview", &query("over api")),
        Some(Tier::WordPrefix)
    );
    assert_eq!(
        tier("Paycheckout", &query("checkout")),
        Some(Tier::Substring)
    );
}

#[test]
fn every_term_must_match() {
    assert_eq!(tier("Checkout API", &query("checkout worker")), None);
}

#[test]
fn no_terms_match_everything() {
    assert_eq!(tier("anything", &[]), Some(Tier::Substring));
}

#[test]
fn better_tiers_come_first() {
    assert_eq!(
        ranked(
            &[
                "Paycheckout",
                "Payments checkout",
                "Checkout overview",
                "checkout"
            ],
            "checkout"
        ),
        [
            "checkout",
            "Checkout overview",
            "Payments checkout",
            "Paycheckout"
        ]
    );
}

#[test]
fn multi_word_queries_match_in_any_order() {
    assert_eq!(
        ranked(
            &["Worker latency", "Payments worker latency", "Checkout API"],
            "latency worker"
        ),
        ["Payments worker latency", "Worker latency"]
    );
}

#[test]
fn score_breaks_ties_within_a_tier_then_the_tie_breaker() {
    let items = vec![("alpha api", 0.0), ("beta api", 2.0), ("gamma api", 2.0)];

    let ranked = rank(
        items,
        &query("api"),
        |(name, _)| name,
        |(_, score)| *score,
        |a, b| b.0.cmp(a.0),
    );

    assert_eq!(
        ranked.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        ["gamma api", "beta api", "alpha api"]
    );
}

#[test]
fn an_empty_query_orders_by_score_then_tie_breaker() {
    let items = vec![("b", 0.0), ("a", 0.0), ("c", 5.0)];

    let ranked = rank(
        items,
        &[],
        |(name, _)| name,
        |(_, score)| *score,
        |a, b| a.0.cmp(b.0),
    );

    assert_eq!(
        ranked.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        ["c", "a", "b"]
    );
}
