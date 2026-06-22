use leptos::prelude::*;

use crate::state::app::use_app_state;

#[component]
pub fn RouteView() -> impl IntoView {
    let state = use_app_state();
    let refresh = {
        let state = state.clone();
        move |_| state.refresh()
    };
    view! {
        <section class="route-panel">
            <div class="route-heading">
                <h1>"Overview"</h1>
                <button type="button" on:click=refresh>"Refresh"</button>
            </div>
            <div class="metric-grid">
                <article>
                    <span>"Scheduler"</span>
                    <strong>{move || state.resources.with(|resources| {
                        resources.status.as_ref()
                            .map(|status| format!("{} running / {} queued", status.scheduler.running, status.scheduler.queued))
                            .unwrap_or_else(|| "-".to_owned())
                    })}</strong>
                </article>
                <article>
                    <span>"Runtimes"</span>
                    <strong>{move || state.resources.with(|resources| {
                        resources.status.as_ref()
                            .map(|status| format!("{} available / {} unavailable", status.runtimes.available, status.runtimes.unavailable))
                            .unwrap_or_else(|| format!("{} loaded", resources.runtimes.len()))
                    })}</strong>
                </article>
                <article>
                    <span>"Permissions"</span>
                    <strong>{move || state.resources.with(|resources| {
                        resources.status.as_ref()
                            .map(|status| format!("{} pending", status.permissions.pending))
                            .unwrap_or_else(|| format!("{} pending", resources.permissions.len()))
                    })}</strong>
                </article>
            </div>
        </section>
    }
}
