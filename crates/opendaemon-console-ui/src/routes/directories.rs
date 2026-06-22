use leptos::prelude::*;
use opendaemon_console_api::dto::DirectoryGrantFormPayload;

use crate::state::app::{
    checkbox_checked, csv, input_value, select_value, use_app_state, workspace_mode,
};

#[component]
pub fn RouteView() -> impl IntoView {
    let state = use_app_state();
    let product_id = RwSignal::new(state.auth.with(|auth| {
        auth.as_ref()
            .and_then(|auth| auth.session.product_id.clone())
            .unwrap_or_default()
    }));
    let agent_id = RwSignal::new(String::new());
    let path = RwSignal::new(String::new());
    let capabilities = RwSignal::new("read, write".to_owned());
    let workspace_worktree = RwSignal::new(true);
    let workspace_direct = RwSignal::new(false);
    let default_mode = RwSignal::new("worktree".to_owned());
    let lock_policy = RwSignal::new("exclusive".to_owned());
    let direct_opt_in = RwSignal::new(true);
    let allow_remote_execution = RwSignal::new(false);
    let save = {
        let state = state.clone();
        move |event: leptos::ev::SubmitEvent| {
            event.prevent_default();
            let mut workspace_modes = Vec::new();
            if workspace_worktree.get() {
                workspace_modes.push(opendaemon_console_api::dto::WorkspaceMode::Worktree);
            }
            if workspace_direct.get() {
                workspace_modes.push(opendaemon_console_api::dto::WorkspaceMode::Direct);
            }
            state.create_directory(DirectoryGrantFormPayload {
                product_id: product_id.get(),
                agent_id: agent_id.get(),
                path: path.get(),
                capabilities: csv(&capabilities.get()),
                workspace_modes,
                default_workspace_mode: workspace_mode(&default_mode.get()),
                lock_policy: lock_policy.get(),
                direct_mode_requires_explicit_task_opt_in: direct_opt_in.get(),
                allow_remote_execution: allow_remote_execution.get(),
            });
        }
    };
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Directories"</h1></div>
            <form class="form-grid wide-form" on:submit=save>
                <input name="product_id" placeholder="Product" prop:value=move || product_id.get() on:input=move |event| product_id.set(input_value(event)) />
                <input name="agent_id" placeholder="Agent" on:input=move |event| agent_id.set(input_value(event)) />
                <input name="path" placeholder="/absolute/local/path" on:input=move |event| path.set(input_value(event)) />
                <input name="capabilities" prop:value=move || capabilities.get() placeholder="read, write" on:input=move |event| capabilities.set(input_value(event)) />
                <fieldset class="choice-row">
                    <label><input type="checkbox" name="workspace_worktree" checked=true on:change=move |event| workspace_worktree.set(checkbox_checked(event)) />"Worktree"</label>
                    <label><input type="checkbox" name="workspace_direct" on:change=move |event| workspace_direct.set(checkbox_checked(event)) />"Direct"</label>
                </fieldset>
                <select name="default_workspace_mode" on:change=move |event| default_mode.set(select_value(event))>
                    <option value="worktree">"Worktree"</option>
                    <option value="direct">"Direct"</option>
                </select>
                <input name="lock_policy" prop:value=move || lock_policy.get() on:input=move |event| lock_policy.set(input_value(event)) />
                <label class="checkbox-row"><input type="checkbox" name="direct_mode_task_opt_in" checked=true on:change=move |event| direct_opt_in.set(checkbox_checked(event)) />"Require task opt-in for direct mode"</label>
                <label class="checkbox-row"><input type="checkbox" name="allow_remote_execution" on:change=move |event| allow_remote_execution.set(checkbox_checked(event)) />"Allow remote execution"</label>
                <button type="submit">"Save grant"</button>
            </form>
            <div class="table-shell">
                <table>
                    <thead><tr><th>"Directory"</th><th>"Agent"</th><th>"Mode"</th></tr></thead>
                    <tbody>
                        {move || {
                            let directories = state.resources.with(|resources| resources.directories.clone());
                            if directories.is_empty() {
                                view! { <tr><td colspan="3">"No directories loaded"</td></tr> }.into_any()
                            } else {
                                directories.into_iter().map(|directory| view! {
                                    <tr>
                                        <td><strong>{directory.path}</strong><span>{format!(" {}", directory.id)}</span></td>
                                        <td>{directory.agent_id}</td>
                                        <td>{format!("{:?}", directory.default_workspace_mode)}</td>
                                    </tr>
                                }).collect_view().into_any()
                            }
                        }}
                    </tbody>
                </table>
            </div>
        </section>
    }
}
