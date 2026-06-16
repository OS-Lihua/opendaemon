use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Products"</h1></div>
            <form class="inline-form">
                <input name="id" placeholder="product_id" />
                <input name="display_name" placeholder="Display name" />
                <button type="submit">"Create"</button>
            </form>
            <div class="table-shell">
                <table>
                    <thead><tr><th>"Product"</th><th>"Status"</th><th>"Tokens"</th></tr></thead>
                    <tbody><tr><td colspan="3">"No products loaded"</td></tr></tbody>
                </table>
            </div>
        </section>
    }
}
