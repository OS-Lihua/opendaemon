use leptos::prelude::*;

use crate::state::app::use_app_state;

#[component]
pub fn RouteView() -> impl IntoView {
    let state = use_app_state();
    let detect = {
        let state = state.clone();
        move |_| state.detect_runtimes()
    };
    view! {
        <section class="route-panel">
            <div class="route-heading">
                <h1>"Providers"</h1>
                <button type="button" on:click=detect>"Detect runtimes"</button>
            </div>
            <div class="table-shell">
                <table>
                    <thead><tr><th>"Provider"</th><th>"Runtime"</th><th>"Status"</th></tr></thead>
                    <tbody>
                        {move || {
                            let resources = state.resources.get();
                            if resources.providers.is_empty() && resources.runtimes.is_empty() {
                                return view! { <tr><td colspan="3">"No providers loaded"</td></tr> }.into_any();
                            }
                            resources.providers.into_iter().map(|provider| {
                                let runtimes = resources.runtimes.iter()
                                    .filter(|runtime| runtime.provider_id == provider.id)
                                    .cloned()
                                    .collect::<Vec<_>>();
                                if runtimes.is_empty() {
                                    view! {
                                        <tr>
                                            <td><strong>{provider.display_name}</strong><span>{format!(" {}", provider.id)}</span></td>
                                            <td>"-"</td>
                                            <td>{provider.status}</td>
                                        </tr>
                                    }.into_any()
                                } else {
                                    runtimes.into_iter().map(|runtime| view! {
                                        <tr>
                                            <td><strong>{provider.display_name.clone()}</strong><span>{format!(" {}", provider.id.clone())}</span></td>
                                            <td>{format!("{} {}", runtime.kind, runtime.version.unwrap_or_default())}</td>
                                            <td>{runtime.status}</td>
                                        </tr>
                                    }).collect_view().into_any()
                                }
                            }).collect_view().into_any()
                        }}
                    </tbody>
                </table>
            </div>
        </section>
    }
}
