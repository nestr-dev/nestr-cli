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

fn is_prime(label: &str) -> bool {
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
}
