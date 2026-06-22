use leptos::prelude::*;

use crate::state::app::use_app_state;

#[component]
pub fn Shell(active_route: &'static str, children: Children) -> impl IntoView {
    let state = use_app_state();
    let refresh = {
        let state = state.clone();
        move |_| state.refresh()
    };
    let sign_out = {
        let state = state.clone();
        move |_| state.sign_out()
    };
    view! {
        <div class="app-shell">
            <aside class="sidebar" aria-label="Console navigation">
                <div class="brand-lockup">
                    <strong>"OpenDaemon"</strong>
                    <span>"Console"</span>
                </div>
                <nav>
                    <NavLink href="/console/" route="overview" active_route=active_route label="Overview" />
                    <NavLink href="/console/products" route="products" active_route=active_route label="Products" />
                    <NavLink href="/console/providers" route="providers" active_route=active_route label="Providers" />
                    <NavLink href="/console/agents" route="agents" active_route=active_route label="Agents" />
                    <NavLink href="/console/directories" route="directories" active_route=active_route label="Directories" />
                    <NavLink href="/console/tasks" route="tasks" active_route=active_route label="Tasks" />
                    <NavLink href="/console/permissions" route="permissions" active_route=active_route label="Permissions" />
                    <NavLink href="/console/settings" route="settings" active_route=active_route label="Settings" />
                </nav>
            </aside>
            <section class="shell-content">
                <header class="top-bar">
                    <span>
                        {move || state.auth.with(|auth| {
                            auth.as_ref()
                                .map(|auth| format!(
                                    "{} {}",
                                    auth.stored.credential_mode,
                                    auth.session.product_id.clone().unwrap_or_else(|| "bootstrap".to_owned())
                                ))
                                .unwrap_or_else(|| "Disconnected".to_owned())
                        })}
                    </span>
                    <div class="top-actions">
                        <button type="button" on:click=refresh>"Refresh"</button>
                        <button type="button" on:click=sign_out>"Sign out"</button>
                    </div>
                </header>
                {children()}
            </section>
        </div>
    }
}

#[component]
fn NavLink(
    href: &'static str,
    route: &'static str,
    active_route: &'static str,
    label: &'static str,
) -> impl IntoView {
    let class = if route == active_route { "active" } else { "" };
    view! { <a href=href class=class>{label}</a> }
}
