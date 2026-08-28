# DS Catalog Browser

A minimal, single-page Yew web app that lists the offers currently known to
a DSP Catalog Broker federated catalog: one row per offer, showing its
participant ID, originator, and dataset count, expandable to the id and
name of each dataset in that offer.

It fetches those offers with the real, published
[`edc-federated-catalog-client`](https://github.com/dataspace-rs/edc-federated-catalog-client)
crate (`FederatedCatalogClient::list_offers()`), not the full
[`edc-web-ui`](https://github.com/dataspace-rs/edc-web-ui) /
`edc-web-components` stack. `edc-web-ui` is a general EDC connector
*management* console (assets, policies, contract definitions, OAuth login,
...); this app is deliberately much smaller and read-only, purpose-built for
browsing one federated catalog's offers and nothing else.

## configuration.json

Fetched at runtime from the app's own origin, same pattern as
`dataspace-rs-ui`'s `configuration.json`:

```json
{
  "catalog_path": "/api/management/v4/catalogs/request",
  "bearer_token": "optional bearer token for the broker's management API"
}
```

- `catalog_path` is a **same-origin, relative path** - not a full URL with a
  host. The app builds its catalog request against its own origin at that
  exact path; a reverse proxy in front of the built app (nginx, in this
  project's deployment) is expected to forward that path to the real DSP
  Catalog Broker. That's a deliberate choice to avoid any CORS dependency,
  and is handled by a separate integration/deployment stage, not by this
  app.
- `bearer_token` is optional. When absent, no `Authorization` header is
  sent at all - `FederatedCatalogClient::new` already takes an
  `Option<String>` for this.

`FederatedCatalogClient::list_offers()` always requests
`"{endpoint}/api/management/{version}/catalogs/request"` - it takes a base
`endpoint`, not the full request path. So this app derives the `endpoint` it
hands to the client by stripping that fixed suffix back off `catalog_path`
and prefixing the page's own origin (see `src/endpoint.rs`), which makes the
client's request land on exactly the configured `catalog_path`.

## Running locally

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.22.0-beta.2
trunk serve
```

Then open `http://localhost:8080`. Without a proxy in front of it,
`configuration.json`'s `catalog_path` won't resolve to a real broker; edit
`Trunk.toml`'s commented-out `[[proxy]]` block (or add your own) to forward
that path to a broker reachable from your machine while developing.

## Testing

The offer -> row-data mapping (`src/mapping.rs`) has no `yew` /
`wasm-bindgen` dependency, so it's covered by plain `cargo test` on the host
target, independent of a browser or a running broker:

```bash
cargo test
```

Building the actual app for the browser:

```bash
trunk build --release
```

## Known limitation

`FederatedCatalogClient::list_offers()` treats any response body it can't
parse as JSON as an empty result (`.unwrap_or_default()` inside the crate),
rather than an error - so a broker returning a non-2xx or malformed
response and a broker genuinely holding zero offers both render as "No
offers were returned by the broker," not an error banner. This app's error
state is reserved for what `list_offers()` actually surfaces as `Err`:
transport-level failures such as the broker being unreachable, plus this
app's own failure to fetch or parse `configuration.json`.
