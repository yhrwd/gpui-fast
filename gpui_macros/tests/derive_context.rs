// Distributed with modifications for gpui-fast / HHMMCL; see NOTICE.
#[test]
fn test_derive_context() {
    use gpui::{App, Window};
    use gpui_macros::{AppContext, VisualContext};

    #[derive(AppContext, VisualContext)]
    struct _MyCustomContext<'a, 'b> {
        #[app]
        app: &'a mut App,
        #[window]
        window: &'b mut Window,
    }
}
