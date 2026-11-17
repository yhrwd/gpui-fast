# HHMMCL GPUI Fast 改造说明

基于 GPUI Fast 0.1.17，Windows 渲染后端仍为 D3D11。

## 改造内容

- 新增 `DirectXDynamicTexture`，允许外部 shader／3D 绘制使用 GPUI 的同一个 D3D11 device，将 GPU 纹理直接交给 GPUI 合成，正常帧循环无需像素读回 CPU。
- 使用独立的 deferred context 录制命令，提交时恢复 GPUI 的上下文状态；绘制失败或 panic 时丢弃本次录制。
- 接入动态纹理版本更新、裁剪、透明度和绘制顺序，并提供尺寸查询、设备有效性检查和资源释放。
- 示例增加皮肤 3D 渲染、程序化 shader、实例化粒子、CNC 拟物材质，以及可中断的原生 UI 弹簧动画。
- 示例补齐暂停、减少动态效果和静止后的帧调度，修复输入回调直接请求动画帧导致的崩溃。

底层实现位于 [gpui_windows/src/fast/dynamic_textures.rs](../gpui_windows/src/fast/dynamic_textures.rs)，公开导出位于 [gpui_windows.rs](../gpui_windows/src/gpui_windows.rs)。本次新增的 GPU 渲染接口和 HLSL 示例面向 Windows，其他平台未新增对应适配。

## 示例路径

示例源码独立于主 workspace，位于 [compat/gpu-gallery/](../compat/gpu-gallery/)，用其自带清单构建。

| 内容 | 路径 |
| --- | --- |
| Gallery 入口 | [gpu_gallery.rs](../compat/gpu-gallery/examples/gpu_gallery.rs) |
| 视口接入与帧调度 | [gpu_gallery/app.rs](../compat/gpu-gallery/examples/gpu_gallery/app.rs) |
| 3D 模型与渲染 | [gpu_gallery/renderer.rs](../compat/gpu-gallery/examples/gpu_gallery/renderer.rs) |
| 程序化 shader 与粒子 | [gpu_gallery/gallery.hlsl](../compat/gpu-gallery/examples/gpu_gallery/gallery.hlsl) |
| CNC 材质 | [gpu_gallery/cnc.hlsl](../compat/gpu-gallery/examples/gpu_gallery/cnc.hlsl) |
| CNC 与平面 UI | [gpu_gallery/studies.rs](../compat/gpu-gallery/examples/gpu_gallery/studies.rs) |
| 动画状态与弹簧响应 | [gpu_gallery/motion.rs](../compat/gpu-gallery/examples/gpu_gallery/motion.rs) |
| 独立构建清单 | [compat/gpu-gallery/Cargo.toml](../compat/gpu-gallery/Cargo.toml) |
