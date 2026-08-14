//! CSR entry. Styles injected from Rust. No authored JavaScript.

use secd_ui::app::App;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("panic: {info}").into());
    }));
    secd_ui::inject_styles();
    leptos::mount::mount_to_body(App);
}
