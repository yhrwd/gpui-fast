// Distributed with modifications for gpui-fast / HHMMCL; see NOTICE.
#[test]
fn test_derive_render() {
    use gpui_macros::Render;

    #[derive(Render)]
    struct _Element;
}
