use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading">
                <h1>"Providers"</h1>
                <button type="button">"Detect runtimes"</button>
            </div>
            <div class="table-shell">
                <table>
                    <thead><tr><th>"Provider"</th><th>"Runtime"</th><th>"Status"</th></tr></thead>
                    <tbody><tr><td colspan="3">"No providers loaded"</td></tr></tbody>
                </table>
            </div>
        </section>
    }
}
