//! Release-only HTTPS root-trust smoke for the optional AI HTTP stack.
//!
//! This test uses the same `ureq` + rustls dependency graph as the AI client.
//! It performs a GET with no API key, repository data, or request body.
//! It is ignored in ordinary test runs and explicitly enabled by the release
//! workflow, so offline development remains fully supported.

use std::time::Duration;

#[test]
#[ignore = "requires network access; run explicitly in release qualification"]
fn rustls_verifies_public_https_certificate_chain() {
    const TRUST_URL: &str = "https://api.github.com/rate_limit";
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .user_agent(concat!("RepoDNA-TLS-SMOKE/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    // Certificate validation is enabled by default. A trust-chain or
    // handshake failure is a transport error, not an HTTP status.
    let response = agent
        .get(TRUST_URL)
        .call()
        .expect("HTTPS handshake and platform-independent trust roots must succeed");
    let status = response.status().as_u16();
    assert!(
        (200..500).contains(&status),
        "TLS succeeded but unexpected upstream HTTP status {status}"
    );
}
