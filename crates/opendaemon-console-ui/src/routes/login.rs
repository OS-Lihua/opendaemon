use leptos::prelude::*;

#[component]
pub fn LoginRoute() -> impl IntoView {
    view! {
        <section class="login-panel" aria-labelledby="login-title">
            <h1 id="login-title">"Connect to OpenDaemon"</h1>
            <form class="form-grid">
                <label>
                    <span>"Credential mode"</span>
                    <select name="credential_mode">
                        <option value="product">"Product token"</option>
                        <option value="bootstrap">"Bootstrap token"</option>
                    </select>
                </label>
                <label>
                    <span>"Base URL"</span>
                    <input name="base_url" value="" placeholder="http://127.0.0.1:3000" />
                </label>
                <label>
                    <span>"Bearer token"</span>
                    <input name="token" type="password" />
                </label>
                <button type="submit">"Connect"</button>
            </form>
        </section>
    }
}
