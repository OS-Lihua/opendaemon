use leptos::prelude::*;

use crate::{routes, shell::Shell, state::app::AppState};

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    provide_context(state.clone());
    state.bootstrap_from_storage();
    let route = route_from_location();
    view! {
        {move || {
            if state.auth.with(Option::is_some) {
                view! {
                    <Shell active_route=route>
                        <main class="workspace">
                            <StatusBanner />
                            {route_view(route)}
                        </main>
                    </Shell>
                }.into_any()
            } else {
                view! {
                    <main class="login-workspace">
                        <StatusBanner />
                        {routes::login::LoginRoute()}
                    </main>
                }.into_any()
            }
        }}
    }
}

fn route_from_location() -> &'static str {
    let path = web_sys::window()
        .and_then(|window| window.location().pathname().ok())
        .unwrap_or_else(|| "/console".to_owned());
    route_from_path(&path)
}

#[must_use]
pub fn route_from_path(path: &str) -> &'static str {
    match path.trim_end_matches('/') {
        "/console/products" => "products",
        "/console/providers" => "providers",
        "/console/agents" => "agents",
        "/console/directories" => "directories",
        "/console/tasks" => "tasks",
        "/console/permissions" => "permissions",
        "/console/settings" => "settings",
        _ => "overview",
    }
}

fn route_view(route: &'static str) -> impl IntoView {
    match route {
        "products" => routes::products::RouteView().into_any(),
        "providers" => routes::providers::RouteView().into_any(),
        "agents" => routes::agents::RouteView().into_any(),
        "directories" => routes::directories::RouteView().into_any(),
        "tasks" => routes::tasks::RouteView().into_any(),
        "permissions" => routes::permissions::RouteView().into_any(),
        "settings" => routes::settings::RouteView().into_any(),
        _ => routes::overview::RouteView().into_any(),
    }
}

#[component]
fn StatusBanner() -> impl IntoView {
    let state = crate::state::app::use_app_state();
    view! {
        <div class="status-banner">
            {move || state.loading.get().then_some(view! { <span>"Loading..."</span> })}
            {move || state.notice.get().map(|notice| view! { <strong>{notice}</strong> })}
            {move || state.error.get().map(|error| view! { <strong class="error-text">{error}</strong> })}
        </div>
    }
}
