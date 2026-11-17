# GPU viewports on GPUI Fast 0.1.17

The HHMMCL fork adds `gpui_windows::DirectXDynamicTexture` while retaining the native Windows D3D11 renderer.

## Changes

- Allocate a render target on GPUI's existing device and composite it through the dynamic-texture sprite pipeline, without pixel readback in the normal frame loop.
- Record producer commands on an isolated deferred context and restore GPUI's immediate-context state on submission. Discard recordings on callback errors or panics.
- Integrate texture content invalidation, clipping, opacity, ordering, device checks and resource retirement.
- Extend the gallery with skin meshes, procedural shaders, instanced particles, CNC materials and interruptible native UI animations.
- Handle pause, reduced motion and idle frame scheduling in the gallery; keep animation-frame requests inside render to avoid the input-callback panic.

Implementation: [dynamic_textures.rs](../gpui_windows/src/fast/dynamic_textures.rs). Public export: [gpui_windows.rs](../gpui_windows/src/gpui_windows.rs). The new GPU owner and HLSL gallery target Windows; this change adds no equivalent backend adaptation for other platforms.

## Example paths

The example sources live in a standalone crate, independent of the main workspace.

- Entry: [gpu_gallery.rs](../compat/gpu-gallery/examples/gpu_gallery.rs).
- Viewports and frame scheduling: [app.rs](../compat/gpu-gallery/examples/gpu_gallery/app.rs).
- Model rendering: [renderer.rs](../compat/gpu-gallery/examples/gpu_gallery/renderer.rs).
- Shader and particles: [gallery.hlsl](../compat/gpu-gallery/examples/gpu_gallery/gallery.hlsl).
- CNC material: [cnc.hlsl](../compat/gpu-gallery/examples/gpu_gallery/cnc.hlsl).
- Native UI: [studies.rs](../compat/gpu-gallery/examples/gpu_gallery/studies.rs).
- Motion state and springs: [motion.rs](../compat/gpu-gallery/examples/gpu_gallery/motion.rs).
- Standalone manifest: [compat/gpu-gallery/Cargo.toml](../compat/gpu-gallery/Cargo.toml).

[中文改造说明](hhmmcl-fork.md).
