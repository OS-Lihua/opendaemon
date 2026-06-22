use leptos::prelude::*;
use opendaemon_console_api::dto::PermissionDecision;

use crate::state::app::{input_value, use_app_state};

#[component]
pub fn RouteView() -> impl IntoView {
    let state = use_app_state();
    let reason = RwSignal::new(String::new());
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Permissions"</h1></div>
            <form class="inline-form">
                <input name="reason" placeholder="Reason" on:input=move |event| reason.set(input_value(event)) />
            </form>
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
                        {move || {
                            let permissions = state.resources.with(|resources| resources.permissions.clone());
                            if permissions.is_empty() {
                                view! { <tr><td colspan="6">"No pending permission requests"</td></tr> }.into_any()
                            } else {
                                permissions.into_iter().map(|permission| {
                                    let approve_state = state.clone();
                                    let deny_state = state.clone();
                                    let approve_task_id = permission.task_id.clone();
                                    let approve_request_id = permission.request_id.clone();
                                    let deny_task_id = permission.task_id.clone();
                                    let deny_request_id = permission.request_id.clone();
                                    view! {
                                        <tr>
                                            <td><strong>{permission.summary}</strong><span>{format!(" {}", permission.request_id)}</span></td>
                                            <td>{permission.provider_id}</td>
                                            <td>{permission.permission_kind}</td>
                                            <td>{permission.expires_at.unwrap_or_else(|| "-".to_owned())}</td>
                                            <td>{reason.get()}</td>
                                            <td>
                                                <button type="button" on:click=move |_| {
                                                    approve_state.respond_to_permission(
                                                        approve_task_id.clone(),
                                                        approve_request_id.clone(),
                                                        PermissionDecision::Approve,
                                                        crate::state::app::optional_string(&reason.get()),
                                                    );
                                                }>"Approve"</button>
                                                <button type="button" on:click=move |_| {
                                                    deny_state.respond_to_permission(
                                                        deny_task_id.clone(),
                                                        deny_request_id.clone(),
                                                        PermissionDecision::Deny,
                                                        crate::state::app::optional_string(&reason.get()),
                                                    );
                                                }>"Deny"</button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view().into_any()
                            }
                        }}
                    </tbody>
                </table>
            </div>
        </section>
    }
}
