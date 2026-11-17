//! Dynamic texture support for the DirectX renderer.
//!
//! The core's [`gpui::DynamicTexturePlatform`] contract arrives here with
//! tightly packed BGRA8 rows. Each texture is its own `ID3D11Texture2D` outside
//! the sprite atlas, so an in-place sub-region upload never disturbs the
//! glyphs and icons sharing an atlas texture. Sprites reference one through a
//! reserved [`AtlasTextureId`] range, which [`view`] resolves before the
//! renderer consults its atlas.
//!
//! D3D11 defers `UpdateSubresource` into the command stream and the stream is
//! submitted before the frame that samples the texture, so a sub-region write
//! never races the sampler.
//!
//! The table is process-wide rather than per renderer: every window shares the
//! same `DirectXDevices`, and a texture created for one window must be visible
//! to whichever renderer draws it. The device is replaced when the renderer
//! recovers from a lost device; textures are then recreated on their next
//! upload instead of being handed a stale pointer.

use std::{collections::HashMap, marker::PhantomData, rc::Rc, sync::Arc};

use anyhow::{Context as _, Result};
use gpui::{
    AtlasTextureId, Bounds, DevicePixels, DynamicTexture, DynamicTextureId, DynamicTexturePlatform,
    Pixels, Size, Window, set_dynamic_texture_platform,
};
use parking_lot::{Mutex, RwLock};
use windows::Win32::Graphics::{
    Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_TEXTURE2D_DESC,
        D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
        ID3D11ShaderResourceView, ID3D11Texture2D,
    },
    Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
};

/// BGRA8, matching the scene's polychrome sprite format.
const BYTES_PER_PIXEL: u32 = 4;

struct Entry {
    size: Size<DevicePixels>,
    /// `None` after a lost device: the size survives so the next upload can
    /// rebuild the texture, but the old GPU objects belong to a dead device.
    gpu: Option<(ID3D11Texture2D, ID3D11ShaderResourceView)>,
}

struct DirectXDynamicTextures {
    /// The device and immediate context of the renderer that installed this
    /// backend. Replaced when that renderer recovers from a lost device.
    gpu: RwLock<(ID3D11Device, ID3D11DeviceContext)>,
    textures: Mutex<HashMap<u32, Entry>>,
}

// The COM objects are only ever used under the locks above, and the immediate
// context is only touched from the window thread that uploads frames.
unsafe impl Send for DirectXDynamicTextures {}
unsafe impl Sync for DirectXDynamicTextures {}

static PLATFORM: RwLock<Option<Arc<DirectXDynamicTextures>>> = RwLock::new(None);

/// Installs the dynamic texture backend for `device`.
///
/// Called from the renderer's constructor. The first call wins: every renderer
/// in the process shares one device, and textures must stay on the device that
/// draws them.
pub(crate) fn init(device: &ID3D11Device, device_context: &ID3D11DeviceContext) {
    let mut platform = PLATFORM.write();
    if platform.is_none() {
        let textures = Arc::new(DirectXDynamicTextures {
            gpu: RwLock::new((device.clone(), device_context.clone())),
            textures: Mutex::new(HashMap::new()),
        });
        set_dynamic_texture_platform(textures.clone());
        *platform = Some(textures);
    }
}

/// Points the backend at the recreated device after a device-lost recovery.
///
/// Every texture allocated on the dead device is dropped; the application's
/// next upload recreates it from the size it was created with.
pub(crate) fn handle_device_lost(device: &ID3D11Device, device_context: &ID3D11DeviceContext) {
    let Some(platform) = PLATFORM.read().clone() else {
        return;
    };
    *platform.gpu.write() = (device.clone(), device_context.clone());
    for entry in platform.textures.lock().values_mut() {
        entry.gpu = None;
    }
}

/// The view a sprite of this texture binds, if the id names a live one.
pub(crate) fn view(id: AtlasTextureId) -> Option<[Option<ID3D11ShaderResourceView>; 1]> {
    let platform = PLATFORM.read();
    let platform = platform.as_ref()?;
    let textures = platform.textures.lock();
    let (_, view) = textures.get(&id.index)?.gpu.as_ref()?;
    Some([Some(view.clone())])
}

impl DynamicTexturePlatform for DirectXDynamicTextures {
    fn create_texture(
        &self,
        id: DynamicTextureId,
        size: Size<DevicePixels>,
        bytes: &[u8],
    ) -> Result<()> {
        let (device, device_context) = self.gpu.read().clone();
        let texture = create_texture_2d(&device, size)?;
        let bounds = Bounds {
            origin: gpui::point(DevicePixels(0), DevicePixels(0)),
            size,
        };
        upload(&device_context, &texture, size, bounds, bytes)?;
        let view = create_view(&device, &texture)?;
        self.textures.lock().insert(
            id.atlas_texture_id().index,
            Entry {
                size,
                gpu: Some((texture, view)),
            },
        );
        Ok(())
    }

    fn update_texture(
        &self,
        id: DynamicTextureId,
        bounds: Bounds<DevicePixels>,
        bytes: &[u8],
    ) -> Result<()> {
        let index = id.atlas_texture_id().index;
        let (device, device_context) = self.gpu.read().clone();
        let mut textures = self.textures.lock();
        let entry = textures
            .get_mut(&index)
            .context("dynamic texture was not created")?;
        let size = entry.size;
        if entry.gpu.is_none() {
            // The texture died with its device. Rebuild it empty and let this
            // upload paint the region it carries; the rest stays black until
            // the producer sends a full-frame update.
            let texture = create_texture_2d(&device, size)?;
            let view = create_view(&device, &texture)?;
            entry.gpu = Some((texture, view));
            log::warn!(
                "recreated {size:?} dynamic texture after a lost device; its contents are \
                 incomplete until the next full-frame upload"
            );
        }
        let (texture, _) = entry.gpu.as_ref().expect("texture was just rebuilt");
        upload(&device_context, texture, size, bounds, bytes)
    }

    fn remove_texture(&self, id: DynamicTextureId) {
        self.textures.lock().remove(&id.atlas_texture_id().index);
    }
}

fn create_texture_2d(device: &ID3D11Device, size: Size<DevicePixels>) -> Result<ID3D11Texture2D> {
    create_texture_with_flags(device, size, D3D11_BIND_SHADER_RESOURCE.0 as u32)
}

fn create_texture_with_flags(
    device: &ID3D11Device,
    size: Size<DevicePixels>,
    flags: u32,
) -> Result<ID3D11Texture2D> {
    anyhow::ensure!(
        size.width.0 > 0 && size.height.0 > 0 && size.width.0 <= 16384 && size.height.0 <= 16384,
        "dynamic texture size must be within 1..=16384 pixels"
    );
    let desc = D3D11_TEXTURE2D_DESC {
        Width: size.width.0.max(0) as u32,
        Height: size.height.0.max(0) as u32,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: flags,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    anyhow::ensure!(
        desc.Width > 0 && desc.Height > 0,
        "dynamic texture dimensions must be positive"
    );
    let mut texture: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }
        .context("creating a dynamic texture")?;
    texture.context("the device returned no dynamic texture")
}

fn create_view(
    device: &ID3D11Device,
    texture: &ID3D11Texture2D,
) -> Result<ID3D11ShaderResourceView> {
    let mut view: Option<ID3D11ShaderResourceView> = None;
    unsafe { device.CreateShaderResourceView(texture, None, Some(&mut view)) }
        .context("creating a dynamic texture view")?;
    view.context("the device returned no dynamic texture view")
}

/// Copies packed rows into `bounds` of `texture`.
///
/// `UpdateSubresource` reads `row_pitch * height` bytes from `bytes` for the
/// box below, so a short slice would make the driver read past its end.
fn upload(
    device_context: &ID3D11DeviceContext,
    texture: &ID3D11Texture2D,
    size: Size<DevicePixels>,
    bounds: Bounds<DevicePixels>,
    bytes: &[u8],
) -> Result<()> {
    anyhow::ensure!(
        bounds.origin.x.0 >= 0
            && bounds.origin.y.0 >= 0
            && bounds.size.width.0 > 0
            && bounds.size.height.0 > 0
            && bounds.origin.x.0 + bounds.size.width.0 <= size.width.0
            && bounds.origin.y.0 + bounds.size.height.0 <= size.height.0,
        "dynamic texture update is outside the texture"
    );
    let row_pitch = bounds.size.width.0 as u32 * BYTES_PER_PIXEL;
    let expected = row_pitch as usize * bounds.size.height.0 as usize;
    anyhow::ensure!(
        bytes.len() >= expected,
        "dynamic texture update is shorter than the region"
    );
    unsafe {
        device_context.UpdateSubresource(
            texture,
            0,
            Some(&D3D11_BOX {
                left: bounds.left().0 as u32,
                top: bounds.top().0 as u32,
                front: 0,
                right: bounds.right().0 as u32,
                bottom: bounds.bottom().0 as u32,
                back: 1,
            }),
            bytes.as_ptr() as _,
            row_pitch,
            0,
        );
    }
    Ok(())
}

/// A GPU render target sampled by GPUI's ordinary sprite pipeline, on the
/// Windows renderer's own D3D11 device. No staging, readback, shared handles,
/// image encoding or second graphics device is involved.
///
/// Create after opening a GPUI window. Use `device()` to create producer
/// resources. `render` records commands on a private deferred context, then
/// executes them on GPUI's immediate context with its state restored. All
/// producer bindings must be set each frame. Output must be premultiplied
/// BGRA8 UNORM, like GPUI's existing dynamic textures (opaque alpha is fine).
///
/// The owner is deliberately neither Send nor Sync. Render on the window
/// thread, during paint or an update. After device recovery, `is_current()`
/// becomes false: recreate the owner AND all resources made with its device.
/// Resize by replacing the owner; do not stretch an old target after DPI changes.
/// As with WgpuDynamicTexture, notify other windows displaying this target.
/// Raw COM operations in the producer remain subject to D3D11's safety rules.
pub struct DirectXDynamicTexture {
    platform: Arc<DirectXDynamicTextures>,
    texture: DynamicTexture,
    size: Size<DevicePixels>,
    device: ID3D11Device,
    deferred: ID3D11DeviceContext,
    target: ID3D11RenderTargetView,
    _window_thread: PhantomData<Rc<()>>,
}

impl DirectXDynamicTexture {
    /// Allocates on the already initialized Windows platform device.
    pub fn new(_window: &mut Window, size: Size<DevicePixels>) -> Result<Self> {
        let platform = PLATFORM
            .read()
            .clone()
            .context("open a GPUI window before creating a GPU texture")?;
        Self::create(platform, size)
    }

    fn create(platform: Arc<DirectXDynamicTextures>, size: Size<DevicePixels>) -> Result<Self> {
        let device = platform.gpu.read().0.clone();
        let target = create_texture_with_flags(
            &device,
            size,
            (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET).0 as u32,
        )?;
        let view = create_view(&device, &target)?;
        let mut render_target = None;
        let mut deferred = None;
        unsafe {
            device.CreateRenderTargetView(&target, None, Some(&mut render_target))?;
            device.CreateDeferredContext(0, Some(&mut deferred))?;
        }
        let target_view = render_target.context("creating GPU render target")?;
        let deferred = deferred.context("creating deferred GPU context")?;
        // Allocate metadata last so a failed D3D allocation cannot leak an id.
        let texture = DynamicTexture::new_gpu(size)?;
        platform.textures.lock().insert(
            texture.id().atlas_texture_id().index,
            Entry {
                size,
                gpu: Some((target, view)),
            },
        );
        let owner = Self {
            platform,
            texture,
            size,
            device,
            deferred,
            target: target_view,
            _window_thread: PhantomData,
        };
        owner.record(|context, target| {
            unsafe {
                context.ClearRenderTargetView(target, &[0.; 4]);
            }
            Ok(())
        })?;
        Ok(owner)
    }

    /// Device for creating shaders, vertex/index buffers and depth targets.
    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }

    /// Physical dimensions; recreate when the element's scaled size changes.
    pub fn size(&self) -> Size<DevicePixels> {
        self.size
    }

    /// False after GPUI replaces its graphics device. Recreate producer state.
    pub fn is_current(&self) -> bool {
        self.platform.gpu.read().0 == self.device
    }

    /// Records and submits a complete frame, then invalidates GPUI's cached
    /// content. A callback error discards the recording without submitting it.
    /// No CPU/GPU synchronization is performed. Do not retain or submit work
    /// on the deferred context outside this callback, or recurse into GPUI.
    pub fn render<T>(
        &self,
        window: &mut Window,
        render: impl FnOnce(&ID3D11DeviceContext, &ID3D11RenderTargetView) -> Result<T>,
    ) -> Result<T> {
        let result = self.record(render)?;
        window.refresh_dynamic_texture(self.texture)?;
        Ok(result)
    }

    fn record<T>(
        &self,
        render: impl FnOnce(&ID3D11DeviceContext, &ID3D11RenderTargetView) -> Result<T>,
    ) -> Result<T> {
        let gpu = self.platform.gpu.read();
        anyhow::ensure!(
            gpu.0 == self.device,
            "GPU texture device was replaced; recreate the texture and producer resources"
        );
        let mut recording = Recording {
            context: &self.deferred,
            finished: false,
        };
        let result = render(&self.deferred, &self.target);
        let mut commands = None;
        // FALSE resets the deferred context, including its recording, on both
        // success and failure. TRUE on execution preserves GPUI's bindings and
        // unbinds the producer's render target before GPUI samples it as an SRV.
        unsafe {
            self.deferred
                .FinishCommandList(false, Some(&mut commands))?;
        }
        recording.finished = true;
        let result = result?;
        let commands = commands.context("recording GPU viewport commands")?;
        unsafe {
            gpu.1.ExecuteCommandList(&commands, true);
        }
        Ok(result)
    }

    /// Call during paint; clipping, opacity and painter order belong to GPUI.
    pub fn paint(&self, window: &mut Window, bounds: Bounds<Pixels>) -> Result<()> {
        anyhow::ensure!(self.is_current(), "GPU texture device was replaced");
        window.paint_dynamic_texture(bounds, self.texture)
    }
}

impl Drop for DirectXDynamicTexture {
    fn drop(&mut self) {
        self.platform
            .textures
            .lock()
            .remove(&self.texture.id().atlas_texture_id().index);
        self.texture.remove();
    }
}

// A panicking producer must not leave commands in the next frame's recording.
struct Recording<'a> {
    context: &'a ID3D11DeviceContext,
    finished: bool,
}
impl Drop for Recording<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let mut discarded = None;
            let _ = unsafe { self.context.FinishCommandList(false, Some(&mut discarded)) };
        }
    }
}

#[cfg(test)]
#[path = "dynamic_textures/gpu_tests.rs"]
mod gpu_tests;
