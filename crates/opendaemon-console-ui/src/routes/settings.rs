use leptos::prelude::*;

use crate::state::app::use_app_state;

#[component]
pub fn RouteView() -> impl IntoView {
    let state = use_app_state();
    let sign_out = {
        let state = state.clone();
        move |_| state.sign_out()
    };
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Settings"</h1></div>
            <dl class="settings-list">
                <dt>"Base URL"</dt><dd>{move || state.auth.with(|auth| auth.as_ref().map(|auth| auth.stored.base_url.clone()).unwrap_or_else(|| "-".to_owned()))}</dd>
                <dt>"Credential"</dt><dd>{move || state.auth.with(|auth| auth.as_ref().map(|auth| auth.session.credential_type.clone()).unwrap_or_else(|| "-".to_owned()))}</dd>
                <dt>"Product"</dt><dd>{move || state.auth.with(|auth| auth.as_ref().and_then(|auth| auth.session.product_id.clone()).unwrap_or_else(|| "-".to_owned()))}</dd>
                <dt>"Scopes"</dt><dd>{move || state.auth.with(|auth| auth.as_ref().map(|auth| auth.session.scopes.join(", ")).unwrap_or_else(|| "-".to_owned()))}</dd>
            </dl>
            <button type="button" on:click=sign_out>"Sign out"</button>
        </section>
    }
}
