mod configuration;
mod endpoint;
mod mapping;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("ds-catalog-browser-ui only targets web (wasm32)");
}

#[cfg(target_arch = "wasm32")]
fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    yew::Renderer::<app::App>::new().render();
}

/// The whole UI: one page, no routing. Gated to wasm32 because it talks to
/// `web_sys::window()` and drives a browser `fetch` via `reqwest`; the
/// fetch-and-map pipeline it drives is otherwise identical to what
/// `crate::mapping`'s host-testable unit tests exercise directly.
#[cfg(target_arch = "wasm32")]
mod app {
    use crate::configuration::Configuration;
    use crate::endpoint::derive_client_endpoint;
    use crate::mapping::{map_offers_to_rows, OfferRow};
    use edc_federated_catalog_client::{FederatedCatalogClient, FederatedCatalogClientVersion};
    use std::collections::HashSet;
    use yew::platform::spawn_local;
    use yew::prelude::*;

    /// Where the page currently is in the load-offers pipeline.
    #[derive(Clone, PartialEq)]
    enum LoadState {
        Loading,
        /// `configuration.json` could not be fetched or parsed.
        ConfigError(String),
        /// `FederatedCatalogClient::list_offers()` returned `Err` - a
        /// transport-level failure such as the broker being unreachable.
        FetchError(String),
        Loaded(Vec<OfferRow>),
    }

    #[function_component(App)]
    pub fn app() -> Html {
        let state = use_state(|| LoadState::Loading);
        let expanded = use_state(HashSet::<String>::new);

        {
            let state = state.clone();
            use_effect_with((), move |_| {
                spawn_local(async move {
                    let Some(origin) = current_origin() else {
                        state.set(LoadState::ConfigError(
                            "could not determine the page origin".to_string(),
                        ));
                        return;
                    };

                    let configuration = match fetch_configuration(&origin).await {
                        Ok(configuration) => configuration,
                        Err(message) => {
                            state.set(LoadState::ConfigError(message));
                            return;
                        }
                    };

                    let version = FederatedCatalogClientVersion::V4;
                    let endpoint =
                        derive_client_endpoint(&origin, &configuration.catalog_path, &version.to_string());

                    let client = FederatedCatalogClient::new(
                        reqwest::Client::new(),
                        endpoint,
                        configuration.bearer_token,
                        FederatedCatalogClientVersion::V4,
                    );

                    match client.list_offers().await {
                        Ok(offers) => state.set(LoadState::Loaded(map_offers_to_rows(&offers))),
                        Err(error) => state.set(LoadState::FetchError(error.to_string())),
                    }
                });

                || ()
            });
        }

        html! {
            <div class="page">
                <header>
                    <h1>{ "DS Catalog Browser" }</h1>
                    <p class="subtitle">
                        { "Offers currently known to the DSP Catalog Broker" }
                    </p>
                </header>
                <main>
                    { render_state(&state, &expanded) }
                </main>
            </div>
        }
    }

    fn render_state(state: &LoadState, expanded: &UseStateHandle<HashSet<String>>) -> Html {
        match state {
            LoadState::Loading => html! {
                <p class="status status-loading" role="status">{ "Loading offers..." }</p>
            },
            LoadState::ConfigError(message) => html! {
                <p class="status status-error" role="alert">
                    { "Could not load configuration.json: " }{ message }
                </p>
            },
            LoadState::FetchError(message) => html! {
                <p class="status status-error" role="alert">
                    { "Could not reach the catalog broker: " }{ message }
                </p>
            },
            LoadState::Loaded(rows) if rows.is_empty() => html! {
                <p class="status status-empty">{ "No offers were returned by the broker." }</p>
            },
            LoadState::Loaded(rows) => render_table(rows, expanded),
        }
    }

    fn render_table(rows: &[OfferRow], expanded: &UseStateHandle<HashSet<String>>) -> Html {
        html! {
            <table class="offers">
                <thead>
                    <tr>
                        <th></th>
                        <th>{ "Participant ID" }</th>
                        <th>{ "Originator" }</th>
                        <th>{ "Datasets" }</th>
                    </tr>
                </thead>
                <tbody>
                    { for rows.iter().map(|row| render_offer_rows(row, expanded)) }
                </tbody>
            </table>
        }
    }

    fn render_offer_rows(row: &OfferRow, expanded: &UseStateHandle<HashSet<String>>) -> Html {
        let key = row.participant_id.clone();
        let is_expanded = expanded.contains(&key);
        let can_expand = row.dataset_count > 0;

        let onclick = {
            let expanded = expanded.clone();
            let key = key.clone();
            Callback::from(move |_| {
                let mut next = (*expanded).clone();
                if !next.insert(key.clone()) {
                    next.remove(&key);
                }
                expanded.set(next);
            })
        };

        let toggle = if can_expand {
            html! {
                <button
                    type="button"
                    class="toggle"
                    aria-expanded={is_expanded.to_string()}
                    onclick={onclick}
                >
                    { if is_expanded { "\u{25be}" } else { "\u{25b8}" } }
                </button>
            }
        } else {
            html! { <span class="toggle toggle-disabled">{ "\u{2013}" }</span> }
        };

        html! {
            <>
                <tr class="offer-row">
                    <td>{ toggle }</td>
                    <td>{ &row.participant_id }</td>
                    <td>{ &row.originator }</td>
                    <td>{ row.dataset_count }</td>
                </tr>
                if is_expanded && can_expand {
                    <tr class="dataset-row">
                        <td></td>
                        <td colspan="3">
                            <ul class="datasets">
                                { for row.datasets.iter().map(|dataset| html! {
                                    <li>
                                        <span class="dataset-name">{ &dataset.name }</span>
                                        <span class="dataset-id">{ &dataset.id }</span>
                                    </li>
                                }) }
                            </ul>
                        </td>
                    </tr>
                }
            </>
        }
    }

    fn current_origin() -> Option<String> {
        web_sys::window()?.location().origin().ok()
    }

    async fn fetch_configuration(origin: &str) -> Result<Configuration, String> {
        let response = reqwest::get(format!("{origin}/configuration.json"))
            .await
            .map_err(|error| error.to_string())?;

        response
            .json::<Configuration>()
            .await
            .map_err(|error| error.to_string())
    }
}
