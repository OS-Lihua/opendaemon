use leptos::prelude::*;
use opendaemon_console_api::dto::AgentProfileFormPayload;

use crate::state::app::{
    checkbox_checked, default_execution_policy, input_value, optional_string, provider_config,
    select_value, textarea_value, use_app_state,
};

#[component]
pub fn RouteView() -> impl IntoView {
    let state = use_app_state();
    let id = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let owner_product_id = RwSignal::new(state.auth.with(|auth| {
        auth.as_ref()
            .and_then(|auth| auth.session.product_id.clone())
            .unwrap_or_default()
    }));
    let provider_id = RwSignal::new(String::new());
    let model = RwSignal::new(String::new());
    let permission_mode = RwSignal::new(String::new());
    let instructions = RwSignal::new(String::new());
    let allow_direct_directory = RwSignal::new(false);
    let custom_args = RwSignal::new(String::new());
    let custom_env_keys = RwSignal::new(String::new());
    let mcp_config = RwSignal::new(String::new());
    let save = {
        let state = state.clone();
        move |event: leptos::ev::SubmitEvent| {
            event.prevent_default();
            state.create_agent(AgentProfileFormPayload {
                id: id.get(),
                name: name.get(),
                owner_product_id: owner_product_id.get(),
                provider_id: provider_id.get(),
                model: model.get(),
                instructions: optional_string(&instructions.get()),
                execution_policy: default_execution_policy(allow_direct_directory.get()),
                provider_config: provider_config(
                    permission_mode.get(),
                    custom_args.get(),
                    custom_env_keys.get(),
                    mcp_config.get(),
                ),
            });
        }
    };
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Agents"</h1></div>
            <form class="form-grid wide-form" on:submit=save>
                <input name="id" placeholder="agent_id" on:input=move |event| id.set(input_value(event)) />
                <input name="name" placeholder="Name" on:input=move |event| name.set(input_value(event)) />
                <input
                    name="owner_product_id"
                    placeholder="Product"
                    prop:value=move || owner_product_id.get()
                    on:input=move |event| owner_product_id.set(input_value(event))
                />
                <input name="provider_id" placeholder="Provider" on:input=move |event| provider_id.set(input_value(event)) />
                <input name="model" placeholder="Model" on:input=move |event| model.set(input_value(event)) />
                <select name="permission_mode" on:change=move |event| permission_mode.set(select_value(event))>
                    <option value="">"Default permissions"</option>
                    <option value="ask">"Ask"</option>
                    <option value="auto">"Auto"</option>
                </select>
                <textarea name="instructions" placeholder="Instructions" on:input=move |event| instructions.set(textarea_value(event))></textarea>
                <label class="checkbox-row">
                    <input type="checkbox" name="allow_direct_directory" on:change=move |event| allow_direct_directory.set(checkbox_checked(event)) />
                    "Allow direct directory"
                </label>
                <input name="custom_args" placeholder="Custom args, comma separated" on:input=move |event| custom_args.set(input_value(event)) />
                <input name="custom_env_keys" placeholder="Custom env keys, comma separated" on:input=move |event| custom_env_keys.set(input_value(event)) />
                <textarea name="mcp_config" placeholder="MCP config JSON" on:input=move |event| mcp_config.set(textarea_value(event))></textarea>
                <button type="submit">"Save agent"</button>
            </form>
            <div class="table-shell">
                <table>
                    <thead><tr><th>"Agent"</th><th>"Provider"</th><th>"Model"</th></tr></thead>
                    <tbody>
                        {move || {
                            let agents = state.resources.with(|resources| resources.agents.clone());
                            if agents.is_empty() {
                                view! { <tr><td colspan="3">"No agents loaded"</td></tr> }.into_any()
                            } else {
                                agents.into_iter().map(|agent| view! {
                                    <tr>
                                        <td><strong>{agent.name}</strong><span>{format!(" {}", agent.id)}</span></td>
                                        <td>{agent.provider_id}</td>
                                        <td>{agent.model}</td>
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
