use anyhow::{Context as _, Result};
use gpui::{DevicePixels, Size};
use std::{ffi::CString, io::Cursor, mem::size_of, path::Path};
use windows::{
    Win32::Graphics::{
        Direct3D::{
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            Fxc::{D3DCOMPILE_ENABLE_STRICTNESS, D3DCOMPILE_OPTIMIZATION_LEVEL3, D3DCompile},
        },
        Direct3D11::*,
        Dxgi::{Common::*, IDXGIDevice},
    },
    core::{Interface, PCSTR, s},
};

pub struct FrameParams {
    pub size: Size<DevicePixels>,
    pub mode: usize,
    pub time: f32,
    pub camera: [f32; 4],
}

pub struct Renderer {
    pub device: ID3D11Device,
    screen: ID3D11VertexShader,
    field: ID3D11PixelShader,
    cnc: ID3D11PixelShader,
    model: ID3D11VertexShader,
    lighting: ID3D11PixelShader,
    particles: ID3D11VertexShader,
    particle_color: ID3D11PixelShader,
    skin: ID3D11ShaderResourceView,
    sampler: ID3D11SamplerState,
    layout: ID3D11InputLayout,
    vertices: ID3D11Buffer,
    vertex_count: u32,
    constants: ID3D11Buffer,
    rasterizer: ID3D11RasterizerState,
    depth_state: ID3D11DepthStencilState,
    blend: ID3D11BlendState,
}

fn shader(entry: &str, profile: &str) -> Result<Vec<u8>> {
    let source: &[u8] = if entry == "CncPS" {
        include_bytes!("cnc.hlsl")
    } else {
        include_bytes!("gallery.hlsl")
    };
    let entry = CString::new(entry)?;
    let profile = CString::new(profile)?;
    let mut code = None;
    let mut error = None;
    let result = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            s!("gallery.hlsl"),
            None,
            None,
            PCSTR(entry.as_ptr().cast()),
            PCSTR(profile.as_ptr().cast()),
            D3DCOMPILE_ENABLE_STRICTNESS | D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut error),
        )
    };
    if let Err(e) = result {
        let message = error
            .map(|blob| unsafe {
                String::from_utf8_lossy(std::slice::from_raw_parts(
                    blob.GetBufferPointer().cast(),
                    blob.GetBufferSize(),
                ))
                .into_owned()
            })
            .unwrap_or_default();
        anyhow::bail!("shader {}: {e}: {message}", entry.to_string_lossy());
    }
    let code = code.context("D3DCompile returned no bytecode")?;
    Ok(unsafe {
        std::slice::from_raw_parts(code.GetBufferPointer().cast(), code.GetBufferSize()).to_vec()
    })
}

impl Renderer {
    pub fn new(device: &ID3D11Device, skin_path: Option<&Path>, slim: bool) -> Result<Self> {
        unsafe {
            let mut screen = None;
            let mut field = None;
            let mut cnc = None;
            let mut model = None;
            let mut lighting = None;
            let mut particles = None;
            let mut particle_color = None;
            let model_code = shader("ModelVS", "vs_5_0")?;
            device.CreateVertexShader(&shader("ScreenVS", "vs_5_0")?, None, Some(&mut screen))?;
            device.CreatePixelShader(&shader("FieldPS", "ps_5_0")?, None, Some(&mut field))?;
            device.CreatePixelShader(&shader("CncPS", "ps_5_0")?, None, Some(&mut cnc))?;
            device.CreateVertexShader(&model_code, None, Some(&mut model))?;
            device.CreatePixelShader(&shader("ModelPS", "ps_5_0")?, None, Some(&mut lighting))?;
            device.CreateVertexShader(
                &shader("ParticleVS", "vs_5_0")?,
                None,
                Some(&mut particles),
            )?;
            device.CreatePixelShader(
                &shader("ParticlePS", "ps_5_0")?,
                None,
                Some(&mut particle_color),
            )?;
            let image = match skin_path {
                Some(path) => {
                    image::open(path).with_context(|| format!("opening skin {}", path.display()))?
                }
                None => image::load_from_memory(include_bytes!("explorer.png"))?,
            }
            .to_rgba8();
            anyhow::ensure!(
                image.width() == image.height()
                    && image.width() >= 64
                    && image.width() % 64 == 0
                    && image.width() <= 4096,
                "skin must be a square PNG with dimensions 64, 128, ... up to 4096"
            );
            let mut skin_texture = None;
            device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: image.width(),
                    Height: image.height(),
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_IMMUTABLE,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    ..Default::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: image.as_ptr().cast(),
                    SysMemPitch: image.width() * 4,
                    ..Default::default()
                }),
                Some(&mut skin_texture),
            )?;
            let mut skin = None;
            device.CreateShaderResourceView(
                skin_texture.as_ref().unwrap(),
                None,
                Some(&mut skin),
            )?;
            let mut sampler = None;
            device.CreateSamplerState(
                &D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    MaxLOD: f32::MAX,
                    ..Default::default()
                },
                Some(&mut sampler),
            )?;
            let elements = [
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("POSITION"),
                    Format: DXGI_FORMAT_R32G32B32_FLOAT,
                    ..Default::default()
                },
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("NORMAL"),
                    Format: DXGI_FORMAT_R32G32B32_FLOAT,
                    AlignedByteOffset: 12,
                    ..Default::default()
                },
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("TEXCOORD"),
                    Format: DXGI_FORMAT_R32_FLOAT,
                    AlignedByteOffset: 24,
                    ..Default::default()
                },
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("TEXCOORD"),
                    SemanticIndex: 1,
                    Format: DXGI_FORMAT_R32G32_FLOAT,
                    AlignedByteOffset: 28,
                    ..Default::default()
                },
            ];
            let mut layout = None;
            device.CreateInputLayout(&elements, &model_code, Some(&mut layout))?;
            let mesh = avatar(slim)?;
            let vertex_count = mesh.len() as u32;
            let mut vertices = None;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: (mesh.len() * size_of::<[f32; 9]>()) as u32,
                    Usage: D3D11_USAGE_IMMUTABLE,
                    BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
                    ..Default::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: mesh.as_ptr().cast(),
                    ..Default::default()
                }),
                Some(&mut vertices),
            )?;
            let mut constants = None;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: 32,
                    Usage: D3D11_USAGE_DYNAMIC,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut constants),
            )?;
            let mut rasterizer = None;
            device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )?;
            let mut depth_state = None;
            device.CreateDepthStencilState(
                &D3D11_DEPTH_STENCIL_DESC {
                    DepthEnable: true.into(),
                    DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                    DepthFunc: D3D11_COMPARISON_LESS,
                    ..Default::default()
                },
                Some(&mut depth_state),
            )?;
            let mut blend = None;
            let mut blend_desc = D3D11_BLEND_DESC::default();
            blend_desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            device.CreateBlendState(&blend_desc, Some(&mut blend))?;
            Ok(Self {
                device: device.clone(),
                screen: screen.unwrap(),
                field: field.unwrap(),
                cnc: cnc.unwrap(),
                model: model.unwrap(),
                lighting: lighting.unwrap(),
                particles: particles.unwrap(),
                particle_color: particle_color.unwrap(),
                skin: skin.unwrap(),
                sampler: sampler.unwrap(),
                layout: layout.unwrap(),
                vertices: vertices.unwrap(),
                vertex_count,
                constants: constants.unwrap(),
                rasterizer: rasterizer.unwrap(),
                depth_state: depth_state.unwrap(),
                blend: blend.unwrap(),
            })
        }
    }

    pub fn adapter(&self) -> Result<String> {
        let desc = unsafe { self.device.cast::<IDXGIDevice>()?.GetAdapter()?.GetDesc()? };
        let end = desc
            .Description
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(desc.Description.len());
        Ok(String::from_utf16_lossy(&desc.Description[..end]))
    }

    pub fn depth(&self, size: Size<DevicePixels>) -> Result<ID3D11DepthStencilView> {
        let mut texture = None;
        let mut view = None;
        unsafe {
            self.device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: size.width.0 as u32,
                    Height: size.height.0 as u32,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_D32_FLOAT,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_DEPTH_STENCIL.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut texture),
            )?;
            self.device
                .CreateDepthStencilView(texture.as_ref().unwrap(), None, Some(&mut view))?;
        }
        view.context("creating depth view")
    }

    pub fn draw(
        &self,
        context: &ID3D11DeviceContext,
        target: &ID3D11RenderTargetView,
        frame: FrameParams,
        depth: Option<&ID3D11DepthStencilView>,
    ) -> Result<()> {
        let FrameParams {
            size,
            mode,
            time,
            camera,
        } = frame;
        unsafe {
            let frame = [
                time,
                size.width.0 as f32,
                size.height.0 as f32,
                mode as f32,
                camera[0],
                camera[1],
                camera[2],
                camera[3],
            ];
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(
                &self.constants,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut mapped),
            )?;
            std::ptr::copy_nonoverlapping(
                frame.as_ptr().cast::<u8>(),
                mapped.pData.cast(),
                size_of::<[f32; 8]>(),
            );
            context.Unmap(&self.constants, 0);
            context.ClearRenderTargetView(target, &[0.026, 0.041, 0.067, 1.]);
            context.OMSetRenderTargets(Some(&[Some(target.clone())]), depth);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: size.width.0 as f32,
                Height: size.height.0 as f32,
                MaxDepth: 1.,
                ..Default::default()
            }]));
            context.RSSetState(&self.rasterizer);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            match mode {
                0 => {
                    context.VSSetShader(&self.screen, None);
                    context.PSSetShader(&self.field, None);
                    context.Draw(3, 0);
                }
                1 => {
                    let depth = depth.context("model needs a depth attachment")?;
                    context.ClearDepthStencilView(depth, D3D11_CLEAR_DEPTH.0, 1., 0);
                    context.OMSetDepthStencilState(&self.depth_state, 0);
                    context.IASetInputLayout(&self.layout);
                    context.IASetVertexBuffers(
                        0,
                        1,
                        Some([Some(self.vertices.clone())].as_ptr()),
                        Some(&36),
                        Some(&0),
                    );
                    context.PSSetShaderResources(0, Some(&[Some(self.skin.clone())]));
                    context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
                    context.VSSetShader(&self.model, None);
                    context.PSSetShader(&self.lighting, None);
                    context.Draw(self.vertex_count, 0);
                }
                3 => {
                    context.VSSetShader(&self.screen, None);
                    context.PSSetShader(&self.cnc, None);
                    context.Draw(3, 0);
                }
                _ => {
                    context.OMSetBlendState(&self.blend, None, u32::MAX);
                    context.VSSetShader(&self.particles, None);
                    context.PSSetShader(&self.particle_color, None);
                    context.DrawInstanced(6, 768, 0, 0);
                }
            }
        }
        Ok(())
    }
}

fn avatar(slim: bool) -> Result<Vec<[f32; 9]>> {
    let source: &[u8] = if slim {
        include_bytes!("avatar_slim.obj")
    } else {
        include_bytes!("avatar.obj")
    };
    let (models, _) = tobj::load_obj_buf(
        &mut Cursor::new(source),
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |_| Ok(Default::default()),
    )?;
    let mut output = Vec::new();
    for model in &models {
        let mesh = &model.mesh;
        let name = model.name.strip_suffix("_outer").unwrap_or(&model.name);
        let part = match name {
            "head" => 0,
            "torso" => 1,
            "left_arm" => 2,
            "right_arm" => 3,
            "left_leg" => 4,
            "right_leg" => 5,
            "ground" => 6,
            _ => anyhow::bail!("unknown avatar part"),
        };
        let part = part + if model.name.ends_with("_outer") { 8 } else { 0 };
        anyhow::ensure!(
            mesh.positions.len() == mesh.normals.len()
                && mesh.texcoords.len() * 3 == mesh.positions.len() * 2,
            "avatar needs normals and UVs"
        );
        for index in &mesh.indices {
            let k = *index as usize * 3;
            let uv = *index as usize * 2;
            output.push([
                mesh.positions[k],
                mesh.positions[k + 1],
                mesh.positions[k + 2],
                mesh.normals[k],
                mesh.normals[k + 1],
                mesh.normals[k + 2],
                part as f32,
                mesh.texcoords[uv],
                1. - mesh.texcoords[uv + 1],
            ]);
        }
    }
    anyhow::ensure!(output.len() == 438, "avatar mesh changed unexpectedly");
    Ok(output)
}

/// Explicit one-shot diagnostic ONLY. The interactive render loop never maps a
/// texture or waits for a GPU. Used by --capture-dir for shader verification.
pub fn capture(device: &ID3D11Device, target: &ID3D11RenderTargetView, path: &Path) -> Result<()> {
    unsafe {
        let texture: ID3D11Texture2D = target.GetResource()?.cast()?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        device.CreateTexture2D(&desc, None, Some(&mut staging))?;
        let staging = staging.context("creating diagnostic staging texture")?;
        let context = device.GetImmediateContext()?;
        context.CopyResource(&staging, &texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        let mut rgba = vec![0; (desc.Width * desc.Height * 4) as usize];
        for y in 0..desc.Height as usize {
            let row = std::slice::from_raw_parts(
                mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                desc.Width as usize * 4,
            );
            for (src, dst) in row.as_chunks::<4>().0.iter().zip(
                rgba[y * desc.Width as usize * 4..][..desc.Width as usize * 4]
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut(),
            ) {
                dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
            }
        }
        context.Unmap(&staging, 0);
        image::save_buffer(
            path,
            &rgba,
            desc.Width,
            desc.Height,
            image::ColorType::Rgba8,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn skin_meshes_have_uvs_layers_and_distinct_arm_widths() -> Result<()> {
    let classic = avatar(false)?;
    let slim = avatar(true)?;
    assert_eq!(classic.len(), 438);
    assert!(
        classic
            .iter()
            .all(|v| (0.0..=1.0).contains(&v[7]) && (0.0..=1.0).contains(&v[8]))
    );
    assert_eq!(classic.iter().filter(|v| v[6] >= 8.).count(), 216);
    let arm_width = |vertices: &Vec<[f32; 9]>| {
        let xs: Vec<f32> = vertices
            .iter()
            .filter(|v| v[6] == 2.)
            .map(|v| v[0])
            .collect();
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max)
            - xs.iter().copied().fold(f32::INFINITY, f32::min)
    };
    assert!((arm_width(&classic) - 0.28).abs() < 0.0001);
    assert!((arm_width(&slim) - 0.21).abs() < 0.0001);
    Ok(())
}

#[cfg(test)]
#[test]
fn cnc_material_shader_compiles() {
    assert!(
        !shader("CncPS", "ps_5_0")
            .expect("compile CNC material")
            .is_empty()
    );
}
