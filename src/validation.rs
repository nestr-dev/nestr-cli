//! Input guards. Prime-label validation (ported from nestr-mcp
//! `src/tools/validation.ts`): a Nest may carry at most one "prime" label
//! (project XOR tension XOR role …); more than one is a semantic conflict the API
//! would reject — we catch it early. The one exception is an *implied* prime:
//! `userstory` implies `project` in the data model, so that pair is one identity,
//! not two (see `PRIME_IMPLICATIONS`). Also hosts the transport-security guard that
//! refuses to send credentials over a non-confidential channel.

use anyhow::{bail, Result};
use url::{Host, Url};

/// Labels that define what a Nest fundamentally *is*. Kept sorted for binary_search.
/// The Scrum labels (epic/milestone/sprint/userstory) count too: a nest is a story
/// OR a sprint OR an epic OR a milestone — stories relate to the containers via
/// graph links, not by sharing labels.
const PRIME_LABELS: &[&str] = &[
    "anchor-circle",
    "checklist",
    "circle",
    "epic",
    "feedback",
    "goal",
    "meeting",
    "metric",
    "milestone",
    "project",
    "result",
    "role",
    "sprint",
    "tension",
    "userstory",
];

/// Prime labels that *imply* another prime label, so the pair is legitimate on one
/// nest. Mirrors nestr-mcp's `PRIME_IMPLICATIONS`. `userstory` declares
/// `implies: ['project']` in nestr-web (`packages/nestr_scrum/labels.js`) — "multi-labeled
/// with `project` so it inherits status, completion, and workspace-wide reporting" — and
/// the server adds `project` itself whenever the story label lands
/// (`imports/api/nests/nest-prototype.js`). It is the only implication in the data model:
/// `sprint`, `epic` and `milestone` declare none and must not share a nest with `project`.
const PRIME_IMPLICATIONS: &[(&str, &str)] = &[("userstory", "project")];

/// True if `label` is a prime label — one that defines what a Nest fundamentally *is*.
pub fn is_prime(label: &str) -> bool {
    PRIME_LABELS.binary_search(&label).is_ok()
}

/// The prime label that `label` implies, if any.
fn implied_by(label: &str) -> Option<&'static str> {
    PRIME_IMPLICATIONS
        .iter()
        .find(|(from, _)| *from == label)
        .map(|(_, to)| *to)
}

/// The distinct prime labels in `labels`, in first-seen order.
fn distinct_primes(labels: &[String]) -> Vec<&str> {
    let mut seen: Vec<&str> = Vec::new();
    for l in labels.iter().map(String::as_str).filter(|l| is_prime(l)) {
        if !seen.contains(&l) {
            seen.push(l);
        }
    }
    seen
}

/// Reject a label set that names more than one prime *identity*. A prime that another
/// present prime implies (`project` alongside `userstory`) is not a second identity.
pub fn validate_prime_labels(labels: &[String]) -> Result<()> {
    let primes = distinct_primes(labels);
    if primes.len() <= 1 {
        return Ok(());
    }
    let implied: Vec<&str> = primes.iter().filter_map(|l| implied_by(l)).collect();
    let effective: Vec<&str> = primes
        .iter()
        .copied()
        .filter(|l| !implied.contains(l))
        .collect();
    if effective.len() > 1 {
        bail!(
            "A nest can have only one prime label, but {} were given: {}. \
             Pick one, or link separate nests instead.",
            primes.len(),
            primes.join(", ")
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
    if let Some(current) = existing.iter().find(|l| {
        is_prime(l)
            && l.as_str() != adding
            // `userstory` and `project` imply one another's presence, either
            // direction — promoting a project to a story is a normal operation.
            && implied_by(adding) != Some(l.as_str())
            && implied_by(l) != Some(adding)
    }) {
        bail!(
            "This nest is already a '{current}' — a nest can have only one prime label. \
             Remove '{current}' first, or use `nests update --label {adding}` to replace the set."
        );
    }
    Ok(())
}

/// Refuse to send credentials over a channel that is not confidential.
///
/// Allows any `https://` URL, and `http://` **only** to a loopback host
/// (`localhost`, `127.0.0.0/8` via `Ipv4Addr::is_loopback`, `::1` via
/// `Ipv6Addr::is_loopback`) so local development against `http://localhost:4001`
/// keeps working. Rejects `http://` to any non-loopback host — where the bearer
/// token or OAuth secrets would travel in cleartext — and every non-`http(s)`
/// scheme. Guarding the API base URL and the OAuth token/authorize endpoints with
/// this resolves the CodeQL `rust/cleartext-transmission` and `rust/non-https-url`
/// findings while keeping loopback dev traffic (which never leaves the machine)
/// untouched.
///
/// Returns the user-facing message as a plain `String` so each call site can wrap
/// it in its own error type (`NestrError::Validation` in `api_client`, `anyhow` in
/// `oauth`) without forcing a conversion between the two.
pub fn require_secure_credential_url(url: &str) -> std::result::Result<(), String> {
    let parsed = Url::parse(url).map_err(|_| {
        format!(
            "refusing to send credentials: '{url}' is not a valid URL, so its transport \
             security cannot be verified. Use an https:// host, or http:// to a loopback \
             host (localhost, 127.0.0.1, or ::1) for local development."
        )
    })?;

    // Real loopback only: `is_loopback()` covers all of 127.0.0.0/8 and ::1; for a
    // domain, accept exactly `localhost` — no DNS resolution, no substring/suffix
    // tricks, so `127.0.0.1.evil.com` and `localhost.evil.com` are *not* loopback.
    let is_loopback = match parsed.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    let shown = parsed.host_str().unwrap_or(url);

    // `Url::parse` lowercases the scheme, so an uppercase `HTTP://` is matched here too.
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if is_loopback => Ok(()),
        "http" => Err(format!(
            "refusing to send credentials in cleartext to '{shown}': the bearer token would be \
             transmitted over an unencrypted http connection. Use an https:// host, or a loopback \
             host (localhost, 127.0.0.1, or ::1) for local development."
        )),
        other => Err(format!(
            "refusing to send credentials over '{other}://' to '{shown}': only https is allowed \
             (or http to a loopback host such as localhost, 127.0.0.1, or ::1 for local development)."
        )),
    }
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
    fn scrum_labels_are_prime() {
        // Shipped with the Scrum app; missing here meant `--label sprint --label project`
        // slipped past the one-prime guard (issue #35, related gap).
        for l in ["epic", "milestone", "sprint", "userstory"] {
            assert!(is_prime(l), "{l} must be a prime label");
        }
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
    fn allows_userstory_with_its_implied_project() {
        // `userstory` declares `implies: ['project']` in nestr-web
        // (packages/nestr_scrum/labels.js), and the server *adds* `project`
        // itself on every story (imports/api/nests/nest-prototype.js). Rejecting
        // the pair made the CLI unable to express what the server generates.
        assert!(validate_prime_labels(&["userstory".into(), "project".into()]).is_ok());
        assert!(validate_prime_labels(&["project".into(), "userstory".into()]).is_ok());
    }

    #[test]
    fn implication_does_not_excuse_an_unrelated_third_prime() {
        let err = validate_prime_labels(&["userstory".into(), "project".into(), "tension".into()])
            .unwrap_err();
        assert!(err.to_string().contains("only one prime label"));
    }

    #[test]
    fn sprint_epic_milestone_still_conflict_with_project() {
        // Only `userstory` carries an implication; these containers must not
        // share a nest with `project`, and stories link to them via graph links.
        for l in ["sprint", "epic", "milestone"] {
            assert!(
                validate_prime_labels(&[l.into(), "project".into()]).is_err(),
                "{l} + project must still be rejected"
            );
        }
    }

    #[test]
    fn a_repeated_prime_is_not_a_conflict() {
        // `--label project --label project` names one identity, not two.
        // nestr-mcp's `findPrimeLabels` dedups via a Set; match that.
        assert!(validate_prime_labels(&["project".into(), "project".into()]).is_ok());
        assert!(
            validate_prime_labels(&["userstory".into(), "userstory".into(), "project".into()])
                .is_ok()
        );
    }

    #[test]
    fn adding_userstory_to_a_project_is_allowed() {
        // Promoting an existing project to a story — a routine Scrum operation.
        assert!(validate_added_prime(&["project".into()], "userstory").is_ok());
    }

    #[test]
    fn adding_project_to_a_userstory_is_allowed() {
        assert!(validate_added_prime(&["userstory".into()], "project").is_ok());
    }

    #[test]
    fn adding_a_container_label_to_a_project_still_conflicts() {
        for l in ["sprint", "epic", "milestone"] {
            assert!(
                validate_added_prime(&["project".into()], l).is_err(),
                "adding {l} to a project must still be rejected"
            );
        }
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

    #[test]
    fn secure_url_allows_https_anywhere() {
        // The guard enforces transport confidentiality, not a host allowlist: https to
        // any host is fine because the channel is encrypted.
        assert!(require_secure_credential_url("https://app.nestr.io/api").is_ok());
        assert!(require_secure_credential_url("https://app.nestr.io/oauth/token").is_ok());
        assert!(require_secure_credential_url("https://anywhere.example/api").is_ok());
    }

    #[test]
    fn secure_url_allows_http_to_loopback() {
        // Default local-dev profile, wiremock's 127.0.0.1, bracketed ::1, and the
        // whole 127.0.0.0/8 range must all stay allowed over plain http.
        assert!(require_secure_credential_url("http://localhost:4001/api").is_ok());
        assert!(require_secure_credential_url("http://127.0.0.1:8080/api").is_ok());
        assert!(require_secure_credential_url("http://[::1]:4001/api").is_ok());
        assert!(require_secure_credential_url("http://127.0.0.5/api").is_ok());
        // url::Url normalizes alternate encodings to the real IP before we check it, so
        // these resolve to 127.0.0.1 and are correctly allowed (traffic stays on-host).
        assert!(require_secure_credential_url("http://127.1/api").is_ok());
        assert!(require_secure_credential_url("http://2130706433/api").is_ok());
        assert!(require_secure_credential_url("http://LOCALHOST/api").is_ok());
    }

    #[test]
    fn secure_url_rejects_http_to_remote() {
        let err = require_secure_credential_url("http://app.nestr.io/api").unwrap_err();
        assert!(
            err.contains("cleartext"),
            "message should explain the risk: {err}"
        );
        assert!(
            err.contains("app.nestr.io"),
            "message should name the host: {err}"
        );
        // 0.0.0.0 is not loopback; an uppercase scheme normalizes to http; a
        // loopback-lookalike domain must not slip through.
        assert!(require_secure_credential_url("http://0.0.0.0/api").is_err());
        assert!(require_secure_credential_url("HTTP://app.nestr.io/api").is_err());
        assert!(require_secure_credential_url("http://127.0.0.1.evil.com/api").is_err());
        // Authority/userinfo spoofs: the real host is what comes after the '@', so a
        // loopback-looking userinfo must NOT smuggle credentials to a remote host.
        assert!(require_secure_credential_url("http://localhost@evil.com/api").is_err());
        assert!(require_secure_credential_url("http://user:pass@evil.com/api").is_err());
        assert!(require_secure_credential_url("http://localhost.evil.com/api").is_err());
    }

    #[test]
    fn secure_url_rejects_non_http_schemes_and_garbage() {
        assert!(require_secure_credential_url("ftp://app.nestr.io/x").is_err());
        assert!(require_secure_credential_url("file:///etc/passwd").is_err());
        assert!(require_secure_credential_url("not a url").is_err());
    }
}
