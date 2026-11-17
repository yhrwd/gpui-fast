// Distributed with modifications for gpui-fast / HHMMCL; see NOTICE.
use gpui::PlatformKeyboardLayout;

pub struct WebKeyboardLayout;

impl PlatformKeyboardLayout for WebKeyboardLayout {
    fn id(&self) -> &str {
        "us"
    }

    fn name(&self) -> &str {
        "US"
    }
}
