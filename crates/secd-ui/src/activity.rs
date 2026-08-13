//! Activity screen: audit metadata only.

use leptos::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRow {
    pub action: String,
    pub name: String,
    pub at: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActivityView {
    pub events: Vec<AuditRow>,
}

#[component]
pub fn ActivityPage(view: ActivityView) -> impl IntoView {
    view! {
        <section data-page="activity">
            <h1>"Activity"</h1>
            <ul data-list="audit">
                {view
                    .events
                    .iter()
                    .map(|e| {
                        view! {
                            <li>
                                <span>{e.action.clone()}</span>
                                <span class="name">{e.name.clone()}</span>
                                <span class="muted">{e.at.clone()}</span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
}

pub fn render_activity(view: &ActivityView) -> String {
    crate::html(|| view! { <ActivityPage view=view.clone() /> })
}
