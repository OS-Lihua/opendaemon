use leptos::prelude::*;
use opendaemon_console_api::dto::TaskCreatePayload;

use crate::state::app::{
    checkbox_checked, csv, input_value, optional_json, textarea_value, use_app_state,
    workspace_mode,
};

#[component]
pub fn RouteView() -> impl IntoView {
    let state = use_app_state();
    let list_state = state.clone();
    let detail_state = state.clone();
    let owner_product_id = RwSignal::new(state.auth.with(|auth| {
        auth.as_ref()
            .and_then(|auth| auth.session.product_id.clone())
            .unwrap_or_default()
    }));
    let agent_id = RwSignal::new(String::new());
    let directory_id = RwSignal::new(String::new());
    let prompt = RwSignal::new(String::new());
    let capabilities = RwSignal::new("read, write".to_owned());
    let workspace = RwSignal::new("worktree".to_owned());
    let direct_opt_in = RwSignal::new(false);
    let metadata = RwSignal::new(String::new());
    let timeout_seconds = RwSignal::new(String::new());
    let create = {
        let state = state.clone();
        move |event: leptos::ev::SubmitEvent| {
            event.prevent_default();
            state.create_task(TaskCreatePayload {
                owner_product_id: owner_product_id.get(),
                agent_id: agent_id.get(),
                directory_id: directory_id.get(),
                prompt: prompt.get(),
                required_capabilities: csv(&capabilities.get()),
                workspace_mode: workspace_mode(&workspace.get()),
                direct_mode_task_opt_in: direct_opt_in.get(),
                metadata: optional_json(&metadata.get()),
                timeout_seconds: timeout_seconds.get().parse().ok(),
            });
        }
    };
    view! {
        <section class="task-layout">
            <div class="route-heading">
                <h1>"Tasks"</h1>
            </div>
            <form class="form-grid wide-form" on:submit=create>
                <input name="owner_product_id" placeholder="Product" prop:value=move || owner_product_id.get() on:input=move |event| owner_product_id.set(input_value(event)) />
                <input name="agent_id" placeholder="Agent" on:input=move |event| agent_id.set(input_value(event)) />
                <input name="directory_id" placeholder="Directory" on:input=move |event| directory_id.set(input_value(event)) />
                <textarea name="prompt" placeholder="Prompt" on:input=move |event| prompt.set(textarea_value(event))></textarea>
                <input name="capabilities" prop:value=move || capabilities.get() on:input=move |event| capabilities.set(input_value(event)) />
                <select name="workspace_mode" on:change=move |event| workspace.set(crate::state::app::select_value(event))>
                    <option value="worktree">"Worktree"</option>
                    <option value="direct">"Direct"</option>
                </select>
                <label class="checkbox-row"><input type="checkbox" on:change=move |event| direct_opt_in.set(checkbox_checked(event)) />"Direct mode opt-in"</label>
                <textarea name="metadata" placeholder="Metadata JSON" on:input=move |event| metadata.set(textarea_value(event))></textarea>
                <input name="timeout_seconds" placeholder="Timeout seconds" on:input=move |event| timeout_seconds.set(input_value(event)) />
                <button type="submit">"Create task"</button>
            </form>
            <div class="task-columns">
                <section class="task-list">
                    <div class="table-shell">
                        <table>
                            <thead><tr><th>"Task"</th><th>"Status"</th><th>"Agent"</th></tr></thead>
                            <tbody>
                                {move || {
                                    let tasks = list_state.resources.with(|resources| resources.tasks.clone());
                                    if tasks.is_empty() {
                                        view! { <tr><td colspan="3">"No tasks loaded"</td></tr> }.into_any()
                                    } else {
                                        tasks.into_iter().map(|task| {
                                            let task_id = task.id.clone();
                                            let select_state = list_state.clone();
                                            view! {
                                                <tr on:click=move |_| select_state.active_task_id.set(Some(task_id.clone()))>
                                                    <td><strong>{task.id}</strong><span>{format!(" {}", task.created_at)}</span></td>
                                                    <td>{format!("{:?}", task.status)}</td>
                                                    <td>{task.agent_id}</td>
                                                </tr>
                                            }
                                        }).collect_view().into_any()
                                    }
                                }}
                            </tbody>
                        </table>
                    </div>
                </section>
                <aside class="task-detail">
                    <h2>"Task detail"</h2>
                    {move || {
                        let active_id = detail_state.active_task_id.get();
                        let task = detail_state.resources.with(|resources| {
                            active_id
                                .as_ref()
                                .and_then(|id| resources.tasks.iter().find(|task| &task.id == id).cloned())
                        });
                        match task {
                            Some(task) => {
                                let cancel_state = detail_state.clone();
                                let cancel_id = task.id.clone();
                                view! {
                                    <div>
                                        <dl>
                                            <dt>"Workspace"</dt><dd>{format!("{:?}", task.workspace_mode)}</dd>
                                            <dt>"Provider"</dt><dd>{task.provider_id.clone()}</dd>
                                            <dt>"Directory"</dt><dd>{task.directory_id.clone()}</dd>
                                            <dt>"Model"</dt><dd>{task.model.clone()}</dd>
                                        </dl>
                                        <button type="button" on:click=move |_| cancel_state.cancel_task(cancel_id.clone())>"Cancel task"</button>
                                        <h3>"Prompt"</h3>
                                        <pre class="result-block">{task.prompt.clone()}</pre>
                                        <h3>"Result"</h3>
                                        <pre class="result-block">{task.result.as_ref().map(|result| result.final_message.clone()).unwrap_or_else(|| "-".to_owned())}</pre>
                                    </div>
                                }.into_any()
                            }
                            None => view! { <p>"Select a task"</p> }.into_any(),
                        }
                    }}
                </aside>
            </div>
        </section>
    }
}
