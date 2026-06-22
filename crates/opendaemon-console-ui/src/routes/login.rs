use leptos::prelude::*;

use crate::state::app::{input_value, select_value, use_app_state};

#[component]
pub fn LoginRoute() -> impl IntoView {
    let state = use_app_state();
    let credential_mode = RwSignal::new("product".to_owned());
    let base_url = RwSignal::new("http://127.0.0.1:19514".to_owned());
    let bearer_token = RwSignal::new(String::new());
    let connect = {
        let state = state.clone();
        move |event: leptos::ev::SubmitEvent| {
            event.prevent_default();
            state.connect(base_url.get(), credential_mode.get(), bearer_token.get());
        }
    };
    view! {
        <section class="login-panel" aria-labelledby="login-title">
            <h1 id="login-title">"Connect to OpenDaemon"</h1>
            <form class="form-grid" on:submit=connect>
                <label>
                    <span>"Credential mode"</span>
                    <select
                        name="credential_mode"
                        on:change=move |event| credential_mode.set(select_value(event))
                    >
                        <option value="product">"Product token"</option>
                        <option value="bootstrap">"Bootstrap token"</option>
                    </select>
                </label>
                <label>
                    <span>"Base URL"</span>
                    <input
                        name="base_url"
                        prop:value=move || base_url.get()
                        on:input=move |event| base_url.set(input_value(event))
                        placeholder="http://127.0.0.1:19514"
                    />
                </label>
                <label>
                    <span>"Bearer token"</span>
                    <input
                        name="token"
                        type="password"
                        on:input=move |event| bearer_token.set(input_value(event))
                    />
                </label>
                <button type="submit">"Connect"</button>
            </form>
        </section>
    }
}
