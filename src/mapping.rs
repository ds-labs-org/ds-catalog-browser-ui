//! Pure, `no_std`-free mapping from the wire shapes returned by
//! `edc_federated_catalog_client::list_offers()` to the flat row-data this
//! UI renders. Deliberately has no `yew` / `wasm-bindgen` / `web-sys`
//! dependency so it can be exercised with plain `cargo test` on the host
//! target, independent of a browser or a running broker.

use edc_federated_catalog_client::models::FederatedCatalogOffer;

/// One row of the top-level offers table.
#[derive(Debug, Clone, PartialEq)]
pub struct OfferRow {
    pub participant_id: String,
    pub originator: String,
    pub dataset_count: usize,
    pub datasets: Vec<DatasetRow>,
}

/// One row in the expanded dataset list nested under an [`OfferRow`].
#[derive(Debug, Clone, PartialEq)]
pub struct DatasetRow {
    pub id: String,
    pub name: String,
}

/// Maps the offers returned by `FederatedCatalogClient::list_offers()` into
/// the row-data this UI renders. Never panics: an offer with zero datasets
/// simply produces an empty `datasets` vec and a `dataset_count` of 0.
pub fn map_offers_to_rows(offers: &[FederatedCatalogOffer]) -> Vec<OfferRow> {
    offers
        .iter()
        .map(|offer| OfferRow {
            participant_id: offer.participant_id.id.clone(),
            originator: offer.originator.clone(),
            dataset_count: offer.dataset.len(),
            datasets: offer
                .dataset
                .iter()
                .map(|dataset| DatasetRow {
                    id: dataset.id.clone(),
                    name: dataset.name.clone(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal but wire-accurate `Policy` object, matching
    /// `edc_connector_client::types::policy::Policy` as re-exported through
    /// `Dataset::has_policy`: `@type` is required (no `#[serde(default)]`
    /// on that field), while `permission`/`obligation`/`prohibition` and the
    /// remaining `Option` fields all have defaults and may be omitted.
    fn minimal_policy_json() -> serde_json::Value {
        serde_json::json!({ "@type": "Set" })
    }

    fn dataset_json(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "@id": id,
            "@type": "dcat:Dataset",
            "http://www.w3.org/ns/odrl/2/hasPolicy": minimal_policy_json(),
            "name": name,
            "contenttype": "application/json",
        })
    }

    fn offer_json(
        offer_id: &str,
        participant_id: &str,
        originator: &str,
        datasets: Vec<serde_json::Value>,
    ) -> serde_json::Value {
        serde_json::json!({
            "@id": offer_id,
            "@type": "Catalog",
            "http://www.w3.org/ns/dcat#dataset": datasets,
            "http://www.w3.org/ns/dcat#service": {
                "@id": format!("{offer_id}-service"),
                "@type": "ConnectorOffer",
                "http://www.w3.org/ns/dcat#endpointDescription": "dspace:connector",
                "http://www.w3.org/ns/dcat#endpointURL": "https://example.com/connector",
            },
            "participantId": { "@id": participant_id },
            "originator": originator,
        })
    }

    fn parse_offers(value: serde_json::Value) -> Vec<FederatedCatalogOffer> {
        serde_json::from_value(value).expect("fixture must match the real wire shape")
    }

    #[test]
    fn maps_an_offer_with_several_datasets() {
        let offers = parse_offers(serde_json::json!([offer_json(
            "offer-1",
            "participant-a",
            "did:web:participant-a",
            vec![
                dataset_json("dataset-1", "First dataset"),
                dataset_json("dataset-2", "Second dataset"),
                dataset_json("dataset-3", "Third dataset"),
            ],
        )]));

        let rows = map_offers_to_rows(&offers);

        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.participant_id, "participant-a");
        assert_eq!(row.originator, "did:web:participant-a");
        assert_eq!(row.dataset_count, 3);
        assert_eq!(
            row.datasets,
            vec![
                DatasetRow {
                    id: "dataset-1".to_string(),
                    name: "First dataset".to_string(),
                },
                DatasetRow {
                    id: "dataset-2".to_string(),
                    name: "Second dataset".to_string(),
                },
                DatasetRow {
                    id: "dataset-3".to_string(),
                    name: "Third dataset".to_string(),
                },
            ]
        );
    }

    #[test]
    fn maps_an_offer_with_zero_datasets_without_panicking() {
        let offers = parse_offers(serde_json::json!([offer_json(
            "offer-2",
            "participant-b",
            "did:web:participant-b",
            vec![],
        )]));

        let rows = map_offers_to_rows(&offers);

        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.participant_id, "participant-b");
        assert_eq!(row.originator, "did:web:participant-b");
        assert_eq!(row.dataset_count, 0);
        assert!(row.datasets.is_empty());
    }

    #[test]
    fn maps_several_offers_in_one_response() {
        let offers = parse_offers(serde_json::json!([
            offer_json("offer-1", "participant-a", "did:web:participant-a", vec![
                dataset_json("dataset-1", "First dataset"),
            ]),
            offer_json("offer-2", "participant-b", "did:web:participant-b", vec![]),
        ]));

        let rows = map_offers_to_rows(&offers);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].dataset_count, 1);
        assert_eq!(rows[1].dataset_count, 0);
    }

    #[test]
    fn maps_an_empty_broker_response() {
        let offers = parse_offers(serde_json::json!([]));

        let rows = map_offers_to_rows(&offers);

        assert!(rows.is_empty());
    }
}
