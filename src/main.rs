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
///
/// Rendered with real `patternfly-yew` components, themed the same way as
/// `dataspace-rs/edc-web-ui` (see that project's `Cargo.toml`/`Trunk.toml`/
/// `index.html` and its `edc-web-components::ListFederatedCatalogOffers`,
/// which this table is modelled on).
#[cfg(target_arch = "wasm32")]
mod app {
    use crate::configuration::Configuration;
    use crate::endpoint::derive_client_endpoint;
    use crate::mapping::{map_offers_to_rows, OfferRow};
    use edc_federated_catalog_client::{FederatedCatalogClient, FederatedCatalogClientVersion};
    use patternfly_yew::prelude::*;
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
                    let endpoint = derive_client_endpoint(
                        &origin,
                        &configuration.catalog_path,
                        &version.to_string(),
                    );

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

        let brand = html!(
            <Title level={Level::H3} size={Size::XXLarge}>{ "DS Catalog Browser" }</Title>
        );

        html!(
            <Page {brand} full_height=true>
                <PageSection>
                    <Stack gutter=true>
                        <StackItem>
                            <Content>
                                <p>{ "Offers currently known to the DSP Catalog Broker" }</p>
                            </Content>
                        </StackItem>
                        <StackItem>
                            { render_state(&state) }
                        </StackItem>
                    </Stack>
                </PageSection>
            </Page>
        )
    }

    fn render_state(state: &LoadState) -> Html {
        match state {
            LoadState::Loading => html!(
                <Bullseye>
                    <Spinner size={SpinnerSize::Xl} aria_label="Loading offers" />
                </Bullseye>
            ),
            LoadState::ConfigError(message) => html!(
                <Alert
                    r#type={AlertType::Danger}
                    title="Could not load configuration.json"
                    inline=true
                >
                    <p>{ message.clone() }</p>
                </Alert>
            ),
            LoadState::FetchError(message) => html!(
                <Alert
                    r#type={AlertType::Danger}
                    title="Could not reach the catalog broker"
                    inline=true
                >
                    <p>{ message.clone() }</p>
                </Alert>
            ),
            LoadState::Loaded(rows) if rows.is_empty() => html!(
                <EmptyState title="No offers" icon={Icon::Cubes}>
                    <p>{ "No offers were returned by the broker." }</p>
                </EmptyState>
            ),
            LoadState::Loaded(rows) => html!(<OffersTable rows={rows.clone()} />),
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

    #[derive(Clone, PartialEq, Properties)]
    struct OffersTableProps {
        rows: Vec<OfferRow>,
    }

    /// The offers table itself, kept as its own component so the
    /// `use_table_data`/`use_memo` hooks it needs to track row-expansion
    /// state are only ever called while offers are actually loaded, never
    /// skipped across renders of `App` (which would otherwise vary the
    /// hook call order between `LoadState` variants).
    #[function_component(OffersTable)]
    fn offers_table(props: &OffersTableProps) -> Html {
        let header = html_nested!(
            <TableHeader<Columns>>
                <TableColumn<Columns> label="Participant ID" index={Columns::ParticipantId} />
                <TableColumn<Columns> label="Originator" index={Columns::Originator} />
                <TableColumn<Columns> label="Datasets" index={Columns::DatasetCount} />
            </TableHeader<Columns>>
        );

        let entries = use_memo(props.rows.clone(), |rows| {
            rows.iter().cloned().map(OfferEntry).collect::<Vec<_>>()
        });
        let (entries, onexpand) = use_table_data(MemoizedTableModel::new(entries));

        html!(
            <Table<Columns, UseTableData<Columns, MemoizedTableModel<OfferEntry>>>
                mode={TableMode::CompactExpandable}
                {header}
                {entries}
                {onexpand}
            />
        )
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Columns {
        ParticipantId,
        Originator,
        DatasetCount,
    }

    /// One offers-table row. Wraps `OfferRow` so it can implement
    /// `TableEntryRenderer` without that (pure, host-testable) type having
    /// to depend on `patternfly-yew` itself.
    #[derive(Clone, Debug, PartialEq)]
    struct OfferEntry(OfferRow);

    impl TableEntryRenderer<Columns> for OfferEntry {
        fn render_cell(&self, context: CellContext<'_, Columns>) -> Cell {
            match context.column {
                Columns::ParticipantId => html!(self.0.participant_id.clone()),
                Columns::Originator => html!(self.0.originator.clone()),
                Columns::DatasetCount => html!(self.0.dataset_count.to_string()),
            }
            .into()
        }

        fn render_details(&self) -> Vec<Span> {
            let content = if self.0.datasets.is_empty() {
                html!(<p>{ "This offer has no datasets." }</p>)
            } else {
                html!(
                    <List r#type={ListType::Plain}>
                        { for self.0.datasets.iter().map(|dataset| html_nested!(
                            <ListItem>
                                <strong>{ &dataset.name }</strong>
                                { " \u{2014} " }
                                <code>{ &dataset.id }</code>
                            </ListItem>
                        )) }
                    </List>
                )
            };

            vec![Span::max(content)]
        }
    }
}
