use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Permissions"</h1></div>
            <div class="table-shell">
                <table>
                    <thead>
                        <tr>
                            <th>"Request"</th>
                            <th>"Provider"</th>
                            <th>"Kind"</th>
                            <th>"Expires"</th>
                            <th>"Reason"</th>
                            <th>"Decision"</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr><td colspan="6">"No pending permission requests"</td></tr>
                    </tbody>
                </table>
            </div>
        </section>
    }
}
