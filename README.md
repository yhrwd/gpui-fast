# HHMMCL GPUI Fast 0.1.17 distribution

> **HHMMCL fork · GPUI Fast 0.1.17** — 新增 Windows 同设备 GPU 渲染接口。改造内容及示例路径见[中文说明](docs/hhmmcl-fork.md)／[English](docs/gpu-viewport.md)。

This branch contains Cargo-consumable release crates generated from the source branch
`hhmmcl/gpu-viewport`, revision `aa50f04dece8e1f14e63f72e496b9271ab9597bc`.

The repository's existing `script/prepare-release.ts --tag v0.1.17` normalizes the
package names to `gpui-fast*`, links the nine sibling crates by local paths, and
uses the same registry support snapshots and compatibility adaptations as the
0.1.17 release. This avoids pulling the source workspace's unreleased Git dependencies
into launcher builds. `HHMMCL-RELEASE.toml` records the source revision.

The Windows backend includes the owning `DirectXDynamicTexture` API from that
source revision. All GPUI core/platform/backend crates are supplied together.

Consumers should use the Git repository and pin a full revision of this distribution
branch, with `package = "gpui-fast"` and `package = "gpui-fast-platform"`.
Use the dependency key `gpui` to preserve macro expansion paths.

The `hhmmcl/gpu-viewport` source branch has been consolidated into this branch:
its commits are ancestors of this branch's history, and development now happens
here directly. The HHMMCL additions beyond the generated distribution — the scroll
layers under `gpui/src/fast/layers/` and `gpui_windows/src/fast/`, and the standalone
GPU gallery under `compat/gpu-gallery/` — live on this branch.

Distributed under Apache-2.0; see LICENSE and NOTICE.
