//! Activity screen: audit metadata only.

use appsy_ui::prelude::*;
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
            <PageHead
                title=ViewFn::from(|| "Activity")
                subtitle=ViewFn::from(|| "Audit metadata. Values are never listed.")
            />
            <Card>
                <CardContent class="secd-list">
                    <div data-list="audit" class="secd-list">
                    {if view.events.is_empty() {
                        view! {
                            <EmptyState
                                title="No events"
                                body=|| "Actions on this vault will show up here."
                            />
                        }.into_any()
                    } else {
                        view! {
                            {view
                                .events
                                .iter()
                                .map(|e| {
                                    let label = format!("{}  {}", e.action, e.name);
                                    let at = e.at.clone();
                                    view! {
                                        <KeyVal
                                            label=label
                                            value=move || at.clone()
                                            mono=true
                                        />
                                    }
                                })
                                .collect_view()}
                        }.into_any()
                    }}
                    </div>
                </CardContent>
            </Card>
        </section>
    }
}

pub fn render_activity(view: &ActivityView) -> String {
    crate::html(|| view! { <ActivityPage view=view.clone() /> })
}
