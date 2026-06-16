use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Settings"</h1></div>
            <dl class="settings-list">
                <dt>"Base URL"</dt><dd>"-"</dd>
                <dt>"Credential"</dt><dd>"-"</dd>
                <dt>"Product"</dt><dd>"-"</dd>
                <dt>"Scopes"</dt><dd>"-"</dd>
            </dl>
            <button type="button">"Sign out"</button>
        </section>
    }
}
