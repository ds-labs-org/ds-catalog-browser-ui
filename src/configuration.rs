use serde::Deserialize;

/// Shape of `configuration.json`, fetched at runtime from the app's own
/// origin (same pattern as `dataspace-rs-ui`'s `configuration.json`).
///
/// `catalog_path` is a same-origin, relative path (e.g.
/// `/api/management/v4/catalogs/request`), never a full URL with a host: a
/// reverse proxy in front of this app is expected to forward that exact
/// path to the real DSP Catalog Broker, so the browser never has to make a
/// cross-origin request.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Configuration {
    pub catalog_path: String,
    #[serde(default)]
    pub bearer_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_token_defaults_to_none_when_absent() {
        let configuration: Configuration =
            serde_json::from_str(r#"{"catalog_path": "/api/management/v4/catalogs/request"}"#)
                .unwrap();

        assert_eq!(
            configuration.catalog_path,
            "/api/management/v4/catalogs/request"
        );
        assert_eq!(configuration.bearer_token, None);
    }

    #[test]
    fn bearer_token_is_read_when_present() {
        let configuration: Configuration = serde_json::from_str(
            r#"{"catalog_path": "/api/management/v4/catalogs/request", "bearer_token": "secret"}"#,
        )
        .unwrap();

        assert_eq!(configuration.bearer_token, Some("secret".to_string()));
    }
}
