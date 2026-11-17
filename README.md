# HHMMCL GPUI Fast 0.1.17 distribution

This branch contains Cargo-consumable release crates generated from the source branch
`hhmmcl/gpu-viewport`, revision `aa50f04dece8e1f14e63f72e496b9271ab9597bc`.
The source branch remains the place to develop and review changes.

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

Regenerate this branch using the existing release preparation workflow in the source
checkout, then commit the generated workspace as this distribution branch's tree.
Keep source changes on the source branch. This distribution does not add a sync checker.

Distributed under Apache-2.0; see LICENSE and NOTICE.
