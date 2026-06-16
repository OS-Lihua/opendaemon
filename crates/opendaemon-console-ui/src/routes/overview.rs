use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading">
                <h1>"Overview"</h1>
                <button type="button">"Refresh"</button>
            </div>
            <div class="metric-grid">
                <article><span>"Scheduler"</span><strong>"0 running"</strong></article>
                <article><span>"Runtimes"</span><strong>"Not detected"</strong></article>
                <article><span>"Permissions"</span><strong>"0 pending"</strong></article>
            </div>
        </section>
    }
}
