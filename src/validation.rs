//! Prime-label validation, ported from nestr-mcp `src/tools/validation.ts`.
//! A Nest may carry at most one "prime" label (project XOR tension XOR role …);
//! more than one is a semantic conflict the API would reject — we catch it early.

use anyhow::{bail, Result};

/// Labels that define what a Nest fundamentally *is*. Kept sorted for binary_search.
const PRIME_LABELS: &[&str] = &[
    "anchor-circle",
    "checklist",
    "circle",
    "feedback",
    "goal",
    "meeting",
    "metric",
    "project",
    "result",
    "role",
    "tension",
];

/// True if `label` is a prime label — one that defines what a Nest fundamentally *is*.
pub fn is_prime(label: &str) -> bool {
    PRIME_LABELS.binary_search(&label).is_ok()
}

/// Reject a label set that names more than one prime label.
pub fn validate_prime_labels(labels: &[String]) -> Result<()> {
    let primes: Vec<&String> = labels.iter().filter(|l| is_prime(l)).collect();
    if primes.len() > 1 {
        let names: Vec<&str> = primes.iter().map(|s| s.as_str()).collect();
        bail!(
            "A nest can have only one prime label, but {} were given: {}. \
             Pick one, or link separate nests instead.",
            names.len(),
            names.join(", ")
        );
    }
    Ok(())
}

/// Reject adding a prime label to a nest that already carries a *different* one.
/// The single-label `nests label add` path bypasses `validate_prime_labels`, so this
/// gives it the same one-prime guarantee — caught early with a friendly message rather
/// than as a raw server rejection. Adding a non-prime label, or re-adding the same prime,
/// is always allowed. `existing` are the nest's current label codes.
pub fn validate_added_prime(existing: &[String], adding: &str) -> Result<()> {
    if !is_prime(adding) {
        return Ok(());
    }
    if let Some(current) = existing
        .iter()
        .find(|l| is_prime(l) && l.as_str() != adding)
    {
        bail!(
            "This nest is already a '{current}' — a nest can have only one prime label. \
             Remove '{current}' first, or use `nests update --label {adding}` to replace the set."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prime_labels_stay_sorted() {
        assert!(
            PRIME_LABELS.is_sorted(),
            "PRIME_LABELS must stay sorted or is_prime's binary_search breaks silently"
        );
    }

    #[test]
    fn allows_zero_or_one_prime() {
        assert!(validate_prime_labels(&[]).is_ok());
        assert!(validate_prime_labels(&["now".into(), "project".into()]).is_ok());
        assert!(validate_prime_labels(&["role".into()]).is_ok());
    }

    #[test]
    fn rejects_two_primes() {
        let err = validate_prime_labels(&["project".into(), "tension".into()]).unwrap_err();
        assert!(err.to_string().contains("only one prime label"));
    }

    #[test]
    fn added_prime_rejected_when_a_different_prime_exists() {
        let err = validate_added_prime(&["role".into()], "project").unwrap_err();
        assert!(err.to_string().contains("already a 'role'"));
    }

    #[test]
    fn added_prime_allowed_without_an_existing_prime() {
        assert!(validate_added_prime(&[], "project").is_ok());
        assert!(validate_added_prime(&["now".into(), "urgent".into()], "project").is_ok());
    }

    #[test]
    fn re_adding_the_same_prime_is_allowed() {
        assert!(validate_added_prime(&["project".into()], "project").is_ok());
    }

    #[test]
    fn adding_a_non_prime_never_conflicts() {
        assert!(validate_added_prime(&["role".into()], "urgent").is_ok());
    }
}
