use leptos::prelude::*;

#[component]
pub fn Shell(active_route: &'static str, children: Children) -> impl IntoView {
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
                    <span>"Local daemon"</span>
                    <strong>"Rust Console"</strong>
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
