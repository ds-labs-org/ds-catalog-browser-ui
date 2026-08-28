//! Derives the `endpoint` argument `FederatedCatalogClient::new` expects
//! from `configuration.json`'s `catalog_path` and the page's own origin.
//!
//! `FederatedCatalogClient::list_offers()` always requests
//! `"{endpoint}/api/management/{version}/catalogs/request"` (see
//! `edc_federated_catalog_client::FederatedCatalogClient::list_offers` in
//! the crate source) - it does not accept the full request path directly.
//! So to make the client hit exactly the configured, same-origin
//! `catalog_path` (and nothing the reverse proxy wasn't told to expect), we
//! strip that fixed suffix back off `catalog_path` before prefixing the
//! origin, rather than concatenating the two blindly.

/// Builds the `endpoint` to pass to `FederatedCatalogClient::new`, such
/// that `list_offers()` ends up requesting `{origin}{catalog_path}`.
///
/// `version_str` is the `Display` form of the
/// `FederatedCatalogClientVersion` in use (e.g. `"v4"`).
pub fn derive_client_endpoint(origin: &str, catalog_path: &str, version_str: &str) -> String {
    let suffix = format!("/api/management/{version_str}/catalogs/request");
    let prefix = catalog_path.strip_suffix(suffix.as_str()).unwrap_or(catalog_path);
    format!("{origin}{prefix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_fixed_suffix_the_client_reappends() {
        let endpoint = derive_client_endpoint(
            "https://catalog.example.com",
            "/api/management/v4/catalogs/request",
            "v4",
        );

        assert_eq!(endpoint, "https://catalog.example.com");
    }

    #[test]
    fn preserves_a_proxy_prefix_ahead_of_the_fixed_suffix() {
        let endpoint = derive_client_endpoint(
            "https://catalog.example.com",
            "/broker/api/management/v4/catalogs/request",
            "v4",
        );

        assert_eq!(endpoint, "https://catalog.example.com/broker");
    }

    #[test]
    fn falls_back_to_a_plain_join_when_the_path_does_not_match_the_expected_suffix() {
        let endpoint = derive_client_endpoint(
            "https://catalog.example.com",
            "/something/unexpected",
            "v4",
        );

        assert_eq!(endpoint, "https://catalog.example.com/something/unexpected");
    }
}
