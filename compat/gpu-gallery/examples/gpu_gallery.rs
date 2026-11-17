//! Native GPU viewport showcase. See docs/gpu-viewport.md.
#[cfg(target_os = "windows")]
extern crate gpui_fast as gpui;
#[cfg(target_os = "windows")]
extern crate gpui_platform_fast as gpui_platform;
#[cfg(target_os = "windows")]
#[path = "gpu_gallery/app.rs"]
mod app;
#[cfg(target_os = "windows")]
fn main() {
    app::run();
}
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!(
        "This example exercises the Windows D3D11 viewport. See gpui_wgpu::WgpuDynamicTexture on wgpu platforms."
    );
}
