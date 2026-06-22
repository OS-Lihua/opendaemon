use leptos::prelude::*;
use opendaemon_console_api::dto::CreateProductPayload;

use crate::state::app::{csv, input_value, optional_string, use_app_state};

#[component]
pub fn RouteView() -> impl IntoView {
    let state = use_app_state();
    let product_id = RwSignal::new(String::new());
    let display_name = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let token_product_id = RwSignal::new(String::new());
    let token_label = RwSignal::new("console-token".to_owned());
    let token_scopes = RwSignal::new(default_scopes().join(", "));
    let create_product = {
        let state = state.clone();
        move |event: leptos::ev::SubmitEvent| {
            event.prevent_default();
            state.create_product(CreateProductPayload {
                id: product_id.get(),
                display_name: display_name.get(),
                description: optional_string(&description.get()),
            });
        }
    };
    let create_token = {
        let state = state.clone();
        move |event: leptos::ev::SubmitEvent| {
            event.prevent_default();
            state.create_product_token(
                token_product_id.get(),
                token_label.get(),
                csv(&token_scopes.get()),
            );
        }
    };
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Products"</h1></div>
            <form class="inline-form" on:submit=create_product>
                <input name="id" placeholder="product_id" on:input=move |event| product_id.set(input_value(event)) />
                <input name="display_name" placeholder="Display name" on:input=move |event| display_name.set(input_value(event)) />
                <input name="description" placeholder="Description" on:input=move |event| description.set(input_value(event)) />
                <button type="submit">"Create"</button>
            </form>
            <form class="inline-form" on:submit=create_token>
                <input name="token_product_id" placeholder="Product ID for token" on:input=move |event| token_product_id.set(input_value(event)) />
                <input name="token_label" prop:value=move || token_label.get() on:input=move |event| token_label.set(input_value(event)) />
                <input name="scopes" prop:value=move || token_scopes.get() on:input=move |event| token_scopes.set(input_value(event)) />
                <button type="submit">"Create token"</button>
            </form>
            {move || state.created_token.get().map(|token| view! {
                <pre class="result-block">{format!("{}\n{}", token.id, token.token)}</pre>
            })}
            <div class="table-shell">
                <table>
                    <thead><tr><th>"Product"</th><th>"Status"</th><th>"Tokens"</th></tr></thead>
                    <tbody>
                        {move || {
                            let products = state.resources.with(|resources| resources.products.clone());
                            if products.is_empty() {
                                view! { <tr><td colspan="3">"No products loaded"</td></tr> }.into_any()
                            } else {
                                products.into_iter().map(|product| view! {
                                    <tr>
                                        <td>
                                            <strong>{product.display_name}</strong>
                                            <span>{format!(" {}", product.id)}</span>
                                        </td>
                                        <td>{product.status}</td>
                                        <td>{product.description.unwrap_or_else(|| "-".to_owned())}</td>
                                    </tr>
                                }).collect_view().into_any()
                            }
                        }}
                    </tbody>
                </table>
            </div>
        </section>
    }
}

fn default_scopes() -> Vec<String> {
    [
        "agents:read",
        "agents:write",
        "directories:read",
        "directories:grant",
        "tasks:read",
        "tasks:create",
        "tasks:cancel",
        "runtimes:read",
        "runtimes:detect",
    ]
    .into_iter()
    .map(ToOwned::to_owned)
    .collect()
}
