use leptos::prelude::*;

#[component]
pub fn RouteView() -> impl IntoView {
    view! {
        <section class="task-layout">
            <div class="route-heading">
                <h1>"Tasks"</h1>
                <button type="button">"Create task"</button>
            </div>
            <div class="task-columns">
                <section class="task-list">
                    <form class="inline-form">
                        <select name="status">
                            <option value="">"All statuses"</option>
                            <option value="queued">"Queued"</option>
                            <option value="running">"Running"</option>
                            <option value="completed">"Completed"</option>
                            <option value="failed">"Failed"</option>
                        </select>
                        <input name="agent_id" placeholder="Agent" />
                        <input name="directory_id" placeholder="Directory" />
                    </form>
                    <div class="table-shell">
                        <table>
                            <thead><tr><th>"Task"</th><th>"Status"</th><th>"Agent"</th></tr></thead>
                            <tbody><tr><td colspan="3">"No tasks loaded"</td></tr></tbody>
                        </table>
                    </div>
                </section>
                <aside class="task-detail">
                    <h2>"Task detail"</h2>
                    <dl>
                        <dt>"Workspace"</dt><dd>"-"</dd>
                        <dt>"Provider"</dt><dd>"-"</dd>
                        <dt>"Session"</dt><dd>"-"</dd>
                    </dl>
                    <h3>"Transcript"</h3>
                    <div class="transcript"></div>
                    <h3>"Result"</h3>
                    <pre class="result-block"></pre>
                </aside>
            </div>
        </section>
    }
}
