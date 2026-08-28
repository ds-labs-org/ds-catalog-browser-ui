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
    /// The dataset's `title` when it carries one, falling back to its `id`
    /// otherwise. This is the label the UI renders as the dataset's
    /// heading, so a title-less dataset renders exactly as it did before
    /// `title` existed on the wire shape.
    pub display_title: String,
    pub description: Option<String>,
    pub version: Option<String>,
    /// Empty when the dataset carries no keywords - never distinguished
    /// from "keywords omitted entirely" since the wire shape doesn't
    /// distinguish those either (`#[serde(default)]` maps both to `vec![]`).
    pub keywords: Vec<String>,
    /// `thumbnail.resource`, expected to be a same-origin path (e.g.
    /// `/assets/datasets/<id>.svg`) the broker serves directly - the UI
    /// `img src`s it as-is, no cross-origin fetch involved.
    pub thumbnail: Option<String>,
}

/// Maps the offers returned by `FederatedCatalogClient::list_offers()` into
/// the row-data this UI renders. Never panics: an offer with zero datasets
/// simply produces an empty `datasets` vec and a `dataset_count` of 0, and
/// every optional dataset field (`title`, `description`, `version`,
/// `keywords`, `thumbnail`) is carried through as-is - present or absent -
/// rather than defaulted to a guessed value.
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
                    display_title: dataset
                        .title
                        .clone()
                        .unwrap_or_else(|| dataset.id.clone()),
                    description: dataset.description.clone(),
                    version: dataset.version.clone(),
                    keywords: dataset.keywords.clone(),
                    thumbnail: dataset.thumbnail.as_ref().map(|t| t.resource.clone()),
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

    /// Builds on [`dataset_json`], layering in the optional fields
    /// `Dataset` carries beyond `name`/`id` - `title`, `description`,
    /// `version`, `keywords`, `thumbnail`, and (only to prove it doesn't
    /// upset parsing even though this UI doesn't surface it) `creator`.
    /// Each parameter left `None`/empty is simply omitted from the JSON, the
    /// same way a broker response that hasn't populated a field yet would
    /// omit it (all six are `#[serde(default)]` on `Dataset`).
    #[allow(clippy::too_many_arguments)]
    fn dataset_json_with(
        id: &str,
        name: &str,
        title: Option<&str>,
        description: Option<&str>,
        version: Option<&str>,
        keywords: &[&str],
        thumbnail: Option<&str>,
        creator: Option<(&str, &str)>,
    ) -> serde_json::Value {
        let mut value = dataset_json(id, name);
        let object = value.as_object_mut().expect("dataset_json returns an object");

        if let Some(title) = title {
            object.insert("title".to_string(), serde_json::json!(title));
        }
        if let Some(description) = description {
            object.insert("description".to_string(), serde_json::json!(description));
        }
        if let Some(version) = version {
            object.insert("version".to_string(), serde_json::json!(version));
        }
        if !keywords.is_empty() {
            object.insert("keywords".to_string(), serde_json::json!(keywords));
        }
        if let Some(resource) = thumbnail {
            object.insert(
                "thumbnail".to_string(),
                serde_json::json!({ "resource": resource }),
            );
        }
        if let Some((creator_name, creator_thumbnail)) = creator {
            object.insert(
                "creator".to_string(),
                serde_json::json!({
                    "name": creator_name,
                    "thumbnail": { "resource": creator_thumbnail },
                }),
            );
        }

        value
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
                    display_title: "dataset-1".to_string(),
                    description: None,
                    version: None,
                    keywords: vec![],
                    thumbnail: None,
                },
                DatasetRow {
                    id: "dataset-2".to_string(),
                    name: "Second dataset".to_string(),
                    display_title: "dataset-2".to_string(),
                    description: None,
                    version: None,
                    keywords: vec![],
                    thumbnail: None,
                },
                DatasetRow {
                    id: "dataset-3".to_string(),
                    name: "Third dataset".to_string(),
                    display_title: "dataset-3".to_string(),
                    description: None,
                    version: None,
                    keywords: vec![],
                    thumbnail: None,
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

    /// A dataset with `title`, `description`, `version`, `keywords`,
    /// `thumbnail` and `creator` all populated - the "every optional field
    /// present" case a real, fully-described broker entry will eventually
    /// look like.
    #[test]
    fn maps_a_dataset_with_every_new_field_populated() {
        let dataset = dataset_json_with(
            "HARVEST-D-01",
            "soil-moisture",
            Some("Soil Moisture Sensor Readings"),
            Some("Time-series volumetric water content readings from in-field probes."),
            Some("2.3.0"),
            &["soil", "moisture", "sensors", "agriculture"],
            Some("/assets/datasets/HARVEST-D-01.svg"),
            Some(("Harvest D Field Systems", "/assets/creators/harvest-d.svg")),
        );
        let offers = parse_offers(serde_json::json!([offer_json(
            "offer-1",
            "participant-a",
            "did:web:participant-a",
            vec![dataset],
        )]));

        let rows = map_offers_to_rows(&offers);

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].datasets,
            vec![DatasetRow {
                id: "HARVEST-D-01".to_string(),
                name: "soil-moisture".to_string(),
                display_title: "Soil Moisture Sensor Readings".to_string(),
                description: Some(
                    "Time-series volumetric water content readings from in-field probes."
                        .to_string()
                ),
                version: Some("2.3.0".to_string()),
                keywords: vec![
                    "soil".to_string(),
                    "moisture".to_string(),
                    "sensors".to_string(),
                    "agriculture".to_string(),
                ],
                thumbnail: Some("/assets/datasets/HARVEST-D-01.svg".to_string()),
            }]
        );
    }

    /// A dataset with none of the new optional fields - the shape every
    /// dataset had before this UI's broker started populating them, and
    /// still what an as-yet-unmigrated participant's offer looks like.
    /// `display_title` must fall back to the id, and every other new field
    /// must come through empty/`None` rather than panicking.
    #[test]
    fn maps_a_dataset_with_no_new_fields() {
        let dataset = dataset_json_with(
            "dataset-1",
            "First dataset",
            None,
            None,
            None,
            &[],
            None,
            None,
        );
        let offers = parse_offers(serde_json::json!([offer_json(
            "offer-1",
            "participant-a",
            "did:web:participant-a",
            vec![dataset],
        )]));

        let rows = map_offers_to_rows(&offers);

        assert_eq!(
            rows[0].datasets,
            vec![DatasetRow {
                id: "dataset-1".to_string(),
                name: "First dataset".to_string(),
                display_title: "dataset-1".to_string(),
                description: None,
                version: None,
                keywords: vec![],
                thumbnail: None,
            }]
        );
    }

    /// A dataset with some but not all new fields populated - title and
    /// version and keywords present, but no thumbnail (and no
    /// description) - proving the fields are carried through independently
    /// rather than all-or-nothing.
    #[test]
    fn maps_a_dataset_with_some_but_not_all_new_fields() {
        let dataset = dataset_json_with(
            "HARVEST-E-01",
            "weather-telemetry",
            Some("Weather Station Telemetry"),
            None,
            Some("4.0.1"),
            &["weather", "telemetry"],
            None,
            None,
        );
        let offers = parse_offers(serde_json::json!([offer_json(
            "offer-1",
            "participant-a",
            "did:web:participant-a",
            vec![dataset],
        )]));

        let rows = map_offers_to_rows(&offers);

        let row = &rows[0].datasets[0];
        assert_eq!(row.id, "HARVEST-E-01");
        assert_eq!(row.display_title, "Weather Station Telemetry");
        assert_eq!(row.description, None);
        assert_eq!(row.version, Some("4.0.1".to_string()));
        assert_eq!(
            row.keywords,
            vec!["weather".to_string(), "telemetry".to_string()]
        );
        assert_eq!(row.thumbnail, None);
    }
}
