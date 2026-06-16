use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Agents"</h1></div>
            <form class="form-grid wide-form">
                <input name="id" placeholder="agent_id" />
                <input name="name" placeholder="Name" />
                <input name="provider_id" placeholder="Provider" />
                <input name="model" placeholder="Model" />
                <select name="permission_mode">
                    <option value="">"Default permissions"</option>
                    <option value="ask">"Ask"</option>
                    <option value="auto">"Auto"</option>
                </select>
                <textarea name="instructions" placeholder="Instructions"></textarea>
                <label class="checkbox-row"><input type="checkbox" name="allow_direct_directory" />"Allow direct directory"</label>
                <input name="custom_args" placeholder="Custom args, comma separated" />
                <input name="custom_env_keys" placeholder="Custom env keys, comma separated" />
                <textarea name="mcp_config" placeholder="MCP config JSON"></textarea>
                <button type="submit">"Save agent"</button>
            </form>
        </section>
    }
}
