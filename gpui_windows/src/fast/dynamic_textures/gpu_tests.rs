// Distributed with modifications for gpui-fast / HHMMCL; see NOTICE.
use super::{DirectXDynamicTexture, DirectXDynamicTextures};
use anyhow::Result;
use gpui::{DevicePixels, size};
use parking_lot::{Mutex, RwLock};
use std::{collections::HashMap, sync::Arc};
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct3D::D3D_DRIVER_TYPE_WARP,
        Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
            D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_USAGE_STAGING, D3D11_VIEWPORT,
            D3D11CreateDevice, ID3D11Texture2D,
        },
    },
};

fn platform() -> Result<Arc<DirectXDynamicTextures>> {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
    }
    Ok(Arc::new(DirectXDynamicTextures {
        gpu: RwLock::new((device.unwrap(), context.unwrap())),
        textures: Mutex::new(HashMap::new()),
    }))
}
fn pixel(owner: &DirectXDynamicTexture) -> Result<[u8; 4]> {
    let texture = owner
        .platform
        .textures
        .lock()
        .get(&owner.texture.id().atlas_texture_id().index)
        .unwrap()
        .gpu
        .as_ref()
        .unwrap()
        .0
        .clone();
    unsafe {
        let mut desc = Default::default();
        texture.GetDesc(&mut desc);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let mut staging: Option<ID3D11Texture2D> = None;
        owner
            .device
            .CreateTexture2D(&desc, None, Some(&mut staging))?;
        let staging = staging.unwrap();
        let gpu = owner.platform.gpu.read();
        gpu.1.CopyResource(&staging, &texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        gpu.1
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        let bytes = std::slice::from_raw_parts(mapped.pData.cast::<u8>(), 4);
        let output = bytes.try_into().unwrap();
        gpu.1.Unmap(&staging, 0);
        Ok(output)
    }
}
#[test]
fn gpu_clear_is_sampleable_and_preserves_immediate_state() -> Result<()> {
    let platform = platform()?;
    let owner =
        DirectXDynamicTexture::create(platform.clone(), size(DevicePixels(8), DevicePixels(8)))?;
    assert_eq!(pixel(&owner)?, [0, 0, 0, 0]);
    unsafe {
        platform.gpu.read().1.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: 123.,
            Height: 456.,
            MaxDepth: 1.,
            ..Default::default()
        }]));
    }
    owner.record(|context, target| {
        unsafe {
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: 8.,
                Height: 8.,
                MaxDepth: 1.,
                ..Default::default()
            }]));
            context.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            context.ClearRenderTargetView(target, &[1., 0., 0., 1.]);
        }
        Ok(())
    })?;
    assert_eq!(pixel(&owner)?, [0, 0, 255, 255]);
    let mut count = 1;
    let mut viewport = D3D11_VIEWPORT::default();
    unsafe {
        platform
            .gpu
            .read()
            .1
            .RSGetViewports(&mut count, Some(&mut viewport));
    }
    assert_eq!(viewport.Width, 123.);
    assert_eq!(viewport.Height, 456.);
    assert_eq!(platform.textures.lock().len(), 1);
    drop(owner);
    assert!(platform.textures.lock().is_empty());
    Ok(())
}
#[test]
fn errors_and_panics_discard_commands_and_allow_reuse() -> Result<()> {
    let owner = DirectXDynamicTexture::create(platform()?, size(DevicePixels(4), DevicePixels(4)))?;
    let result: Result<()> = owner.record(|context, target| {
        unsafe {
            context.ClearRenderTargetView(target, &[0., 1., 0., 1.]);
        }
        anyhow::bail!("producer failed")
    });
    assert!(result.is_err());
    assert_eq!(pixel(&owner)?, [0; 4]);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<()> = owner.record(|context, target| {
            unsafe {
                context.ClearRenderTargetView(target, &[1., 0., 0., 1.]);
            }
            panic!("producer panic")
        });
    }));
    assert!(result.is_err());
    owner.record(|_, _| Ok(()))?;
    assert_eq!(pixel(&owner)?, [0; 4]);
    owner.record(|context, target| {
        unsafe {
            context.ClearRenderTargetView(target, &[0., 0., 1., 1.]);
        }
        Ok(())
    })?;
    assert_eq!(pixel(&owner)?, [255, 0, 0, 255]);
    Ok(())
}
#[test]
fn dimensions_and_device_replacement_are_checked() -> Result<()> {
    let platform = platform()?;
    for (w, h) in [(0, 4), (-1, 4), (4, 0), (16385, 1)] {
        assert!(
            DirectXDynamicTexture::create(platform.clone(), size(DevicePixels(w), DevicePixels(h)))
                .is_err()
        );
    }
    assert!(platform.textures.lock().is_empty());
    let owner =
        DirectXDynamicTexture::create(platform.clone(), size(DevicePixels(4), DevicePixels(4)))?;
    let other = self::platform()?;
    *platform.gpu.write() = other.gpu.read().clone();
    assert!(!owner.is_current());
    let result: Result<()> =
        owner.record(|_, _| panic!("a replaced device must reject before invoking producer"));
    assert!(result.is_err());
    Ok(())
}
