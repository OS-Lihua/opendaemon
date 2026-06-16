use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="route-panel">
            <div class="route-heading"><h1>"Directories"</h1></div>
            <form class="form-grid wide-form">
                <input name="product_id" placeholder="Product" />
                <input name="agent_id" placeholder="Agent" />
                <input name="path" placeholder="/absolute/local/path" />
                <input name="capabilities" placeholder="read, write" />
                <fieldset class="choice-row">
                    <label><input type="checkbox" name="workspace_worktree" checked=true />"Worktree"</label>
                    <label><input type="checkbox" name="workspace_direct" />"Direct"</label>
                </fieldset>
                <select name="default_workspace_mode">
                    <option value="worktree">"Worktree"</option>
                    <option value="direct">"Direct"</option>
                </select>
                <input name="lock_policy" value="exclusive" />
                <label class="checkbox-row"><input type="checkbox" name="direct_mode_task_opt_in" checked=true />"Require task opt-in for direct mode"</label>
                <label class="checkbox-row"><input type="checkbox" name="allow_remote_execution" />"Allow remote execution"</label>
                <button type="submit">"Save grant"</button>
            </form>
        </section>
    }
}
