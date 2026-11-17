use anyhow::Result;
use gpui::{
    App, Bounds, Context, DevicePixels, FocusHandle, Focusable, MouseButton, Pixels, SharedString,
    Window, WindowBounds, WindowOptions, actions, canvas, div, point, prelude::*, px, rgb, size,
};
use gpui_windows::DirectXDynamicTexture;
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Instant};
use windows::Win32::Graphics::Direct3D11::ID3D11DepthStencilView;
#[path = "renderer.rs"]
mod renderer;
use renderer::Renderer;

actions!(gpu_gallery, [TogglePlayback, ResetCamera]);
struct Panel {
    target: DirectXDynamicTexture,
    depth: Option<ID3D11DepthStencilView>,
}
struct GpuState {
    renderer: Option<Renderer>,
    panels: [Option<Panel>; 3],
    error: Option<String>,
    skin_path: Option<PathBuf>,
    slim: bool,
    time: f32,
    camera: [f32; 4],
    capture: Option<PathBuf>,
    captured: [bool; 3],
    frame_cpu_ms: f64,
    samples: Vec<f64>,
    adapter: String,
}
impl GpuState {
    fn paint(&mut self, index: usize, bounds: Bounds<Pixels>, window: &mut Window) -> Result<()> {
        if self.error.is_some() {
            return Ok(());
        }
        let scale = window.scale_factor() * if index == 1 { 2.0 } else { 1.0 };
        let dimensions = size(
            DevicePixels(
                (f32::from(bounds.size.width) * scale)
                    .round()
                    .clamp(1., 4096.) as i32,
            ),
            DevicePixels(
                (f32::from(bounds.size.height) * scale)
                    .round()
                    .clamp(1., 4096.) as i32,
            ),
        );
        let recreate = self.panels[index]
            .as_ref()
            .is_none_or(|p| p.target.size() != dimensions || !p.target.is_current());
        if self.panels.iter().flatten().any(|p| !p.target.is_current()) {
            self.panels = [None, None, None];
            self.renderer = None;
        }
        if recreate || self.panels[index].is_none() {
            let target = DirectXDynamicTexture::new(window, dimensions)?;
            if self.renderer.is_none() {
                let renderer =
                    Renderer::new(target.device(), self.skin_path.as_deref(), self.slim)?;
                self.adapter = renderer.adapter()?;
                eprintln!(
                    "GPU gallery: {} / D3D11 / same-device GPU textures",
                    self.adapter
                );
                self.renderer = Some(renderer);
            }
            let depth = if index == 1 {
                Some(self.renderer.as_ref().unwrap().depth(dimensions)?)
            } else {
                None
            };
            self.panels[index] = Some(Panel { target, depth });
        }
        let panel = self.panels[index].as_ref().unwrap();
        let renderer = self.renderer.as_ref().unwrap();
        let start = Instant::now();
        let target = panel.target.render(window, |context, target| {
            renderer.draw(
                context,
                target,
                renderer::FrameParams {
                    size: dimensions,
                    mode: index,
                    time: self.time,
                    camera: self.camera,
                },
                panel.depth.as_ref(),
            )?;
            Ok(target.clone())
        })?;
        self.frame_cpu_ms += start.elapsed().as_secs_f64() * 1000.;
        if index == 2 {
            if self.samples.len() < 100_000 {
                self.samples.push(self.frame_cpu_ms);
            }
            self.frame_cpu_ms = 0.;
        }
        if let Some(directory) = &self.capture
            && !self.captured[index]
        {
            std::fs::create_dir_all(directory)?;
            renderer::capture(
                &renderer.device,
                &target,
                &directory.join(["shader.png", "model.png", "particles.png"][index]),
            )?;
            self.captured[index] = true;
        }
        panel.target.paint(window, bounds)
    }
}
struct Gallery {
    gpu: Rc<RefCell<GpuState>>,
    last: Instant,
    paused: bool,
    focus: FocusHandle,
    dragging: bool,
    previous: gpui::Point<Pixels>,
    frame_limit: Option<usize>,
    frames: usize,
    report: Option<PathBuf>,
}
impl Focusable for Gallery {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn muted(text: impl Into<SharedString>) -> gpui::Div {
    div().text_sm().text_color(rgb(0x8f9aa9)).child(text.into())
}
fn badge(text: &'static str) -> gpui::Div {
    div()
        .px_3()
        .py_1()
        .rounded_full()
        .bg(rgb(0x26332c))
        .text_color(rgb(0xc4e99a))
        .text_xs()
        .child(text)
}
fn heading(number: &'static str, title: &'static str, tag: &'static str) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(rgb(0x26303e))
        .child(
            div()
                .flex()
                .gap_3()
                .items_center()
                .child(muted(number))
                .child(title),
        )
        .child(badge(tag))
}
impl Gallery {
    fn toggle(&mut self, _: &TogglePlayback, _window: &mut Window, cx: &mut Context<Self>) {
        self.paused = !self.paused;
        self.last = Instant::now();
        cx.notify();
    }
    fn reset(&mut self, _: &ResetCamera, _window: &mut Window, cx: &mut Context<Self>) {
        self.gpu.borrow_mut().camera = [-0.48, -0.16, 1., 1.];
        cx.notify();
    }
    fn viewport(&self, index: usize) -> impl IntoElement {
        let gpu = self.gpu.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let result = gpu.borrow_mut().paint(index, bounds, window);
                if let Err(error) = result {
                    let message = format!("{error:#}");
                    if gpu.borrow().error.as_ref() != Some(&message) {
                        eprintln!("GPU gallery: {message}");
                    }
                    gpu.borrow_mut().error = Some(message);
                }
            },
        )
        .size_full()
    }
    fn report(&self) {
        let gpu = self.gpu.borrow();
        let mut samples: Vec<f64> = gpu.samples.iter().skip(20).copied().collect();
        samples.sort_by(f64::total_cmp);
        let p = |q: f64| {
            samples
                .get(((samples.len().saturating_sub(1)) as f64 * q) as usize)
                .copied()
                .unwrap_or(0.)
        };
        let report = serde_json::json!({"adapter":gpu.adapter,"backend":"D3D11", "viewports":3,
            "frames":self.frames,"samples_after_warmup":samples.len(),"cpu_record_submit_ms_p50":p(0.5),"cpu_record_submit_ms_p95":p(0.95),
            "note":"CPU record+submit for three viewports; excludes GPU execution, GPUI composition and present. No backend comparison.",
            "sizes_physical":gpu.panels.iter().map(|p|p.as_ref().map(|p|[p.target.size().width.0,p.target.size().height.0])).collect::<Vec<_>>(),
            "error":gpu.error});
        eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
        if let Some(path) = &self.report {
            let _ = std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap());
        }
    }
}
impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        if !self.paused {
            self.gpu.borrow_mut().time += now.duration_since(self.last).as_secs_f32().min(0.1);
            window.request_animation_frame();
        }
        self.last = now;
        if self.frame_limit.is_some_and(|limit| self.frames >= limit) {
            self.report();
            cx.quit();
        }
        self.frames += 1;
        let gpu = self.gpu.borrow();
        let error = gpu.error.clone();
        let model_label = if gpu.slim {
            "Model: Slim / 3px"
        } else {
            "Model: Classic / 4px"
        };
        let adapter = if gpu.adapter.is_empty() {
            "Initializing GPU".to_owned()
        } else {
            gpu.adapter.clone()
        };
        drop(gpu);
        let playback = if self.paused {
            "Play / Space"
        } else {
            "Pause / Space"
        };
        div().track_focus(&self.focus).key_context("GpuGallery").on_action(cx.listener(Self::toggle)).on_action(cx.listener(Self::reset))
            .size_full().flex().bg(rgb(0x0b1019)).text_color(rgb(0xe5eaf1)).font_family("Segoe UI").text_sm()
            .child(div().w(px(174.)).h_full().flex().flex_col().p_5().gap_6().border_r_1().border_color(rgb(0x202937))
                .child(div().text_lg().font_weight(gpui::FontWeight::BOLD).child("GPUI / LAB"))
                .child(muted("RENDER EXPLORER"))
                .child(div().px_3().py_3().rounded_lg().bg(rgb(0x222f31)).text_color(rgb(0xc4e99a)).child("01   GPU gallery"))
                .child(muted("Shader fields")).child(div().id("model-type").cursor_pointer().text_color(rgb(0xc4e99a)).child(model_label)
                    .on_click(cx.listener(|this,_,_,cx| { let mut gpu=this.gpu.borrow_mut(); gpu.slim = !gpu.slim; gpu.panels=[None,None,None];gpu.renderer=None;drop(gpu);cx.notify(); }))).child(muted("Particle motion"))
                .child(div().flex_1()).child(muted("FORK BASE"))
                .child(div().child("GPUI Fast 0.1.17")))
            .child(div().flex_1().min_w_0().h_full().flex().flex_col().p_6().gap_4()
                .child(div().flex().items_center().justify_between()
                    .child(div().flex().flex_col().gap_2().child(muted("EXPERIMENTS / NATIVE GRAPHICS"))
                        .child(div().text_3xl().font_weight(gpui::FontWeight::SEMIBOLD).child("Pixels, with depth."))
                        .child(muted("Three live scenes. One graphics device.")))
                    .child(div().id("playback").cursor_pointer().px_4().py_3().rounded_lg().bg(rgb(0xc4e99a)).text_color(rgb(0x142019))
                        .on_click(cx.listener(|this,_,window,cx|this.toggle(&TogglePlayback,window,cx))).child(playback)))
                .child(div().flex_1().min_h_0().flex().gap_4()
                    .child(div().flex_1().min_w_0().flex().flex_col().rounded_xl().overflow_hidden().border_1().border_color(rgb(0x26303e)).bg(rgb(0x0d1420))
                        .child(heading("01","Chromatic field","FRAGMENT SHADER"))
                        .child(div().flex_1().min_h_0().child(self.viewport(0)))
                        .child(div().px_4().py_3().child(muted("Domain warping / interference / procedural color"))))
                    .child(div().w(px(340.)).flex().flex_col().rounded_xl().overflow_hidden().border_1().border_color(rgb(0x26303e)).bg(rgb(0x0d1420))
                        .child(heading("02","Skin preview","3D / 2X AA"))
                        .child(div().id("model-camera").flex_1().min_h_0().cursor_grab()
                            .on_mouse_down(MouseButton::Left,cx.listener(|this,event: &gpui::MouseDownEvent,_,_| {this.dragging=true;this.previous=event.position;}))
                            .on_mouse_up(MouseButton::Left,cx.listener(|this,_,_,_|this.dragging=false))
                            .on_mouse_up_out(MouseButton::Left,cx.listener(|this,_,_,_|this.dragging=false))
                            .on_mouse_move(cx.listener(|this,event: &gpui::MouseMoveEvent,_window,cx| {
                                if this.dragging && event.pressed_button==Some(MouseButton::Left) {
                                    let delta=event.position-this.previous; this.previous=event.position;
                                    let mut gpu=this.gpu.borrow_mut(); gpu.camera[0]+=f32::from(delta.x)*0.009;
                                    gpu.camera[1]=(gpu.camera[1]+f32::from(delta.y)*0.006).clamp(-0.65,0.65);
                                    drop(gpu); cx.notify();
                                }
                            })).on_scroll(cx.listener(|this,event: &gpui::ScrollEvent,_,cx| {
                                let dy=f32::from(event.delta.pixel_delta(px(20.)).y);
                                let mut gpu=this.gpu.borrow_mut();gpu.camera[2]=(gpu.camera[2]*(dy*0.002).exp()).clamp(0.65,1.25);
                                drop(gpu);cx.stop_propagation();cx.notify();
                            }))
                            .child(self.viewport(1)))
                        .child(div().flex().items_center().justify_between().px_4().py_3()
                            .child(muted("Drag / scroll to zoom"))
                            .child(div().id("reset-camera").cursor_pointer().text_color(rgb(0xc4e99a))
                                .on_click(cx.listener(|this,_,window,cx|this.reset(&ResetCamera,window,cx))).child("Reset / R")))))
                .child(div().h(px(222.)).flex().rounded_xl().overflow_hidden().border_1().border_color(rgb(0x26303e)).bg(rgb(0x0d1420))
                    .child(div().w(px(260.)).p_5().flex().flex_col().gap_3()
                        .child(badge("03 / INSTANCED MOTION"))
                        .child(div().text_xl().child("Order into orbit."))
                        .child(muted("768 particles morph between a disk and a trefoil knot with a damped spring response.")))
                    .child(div().flex_1().min_w_0().child(self.viewport(2))))
                .child(div().flex().items_center().justify_between().text_xs().text_color(rgb(0x8f9aa9))
                    .child(if adapter.is_empty() {"Initializing graphics device...".to_owned()} else {format!("D3D11 / {adapter}")})
                    .child("SPACE  Play / Pause     R  Reset camera"))
                .when_some(error,|this,error|this.child(div().text_color(rgb(0xffb4ab)).child(error))))
    }
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let capture = value("--capture-dir").map(PathBuf::from);
    let skin_path = value("--skin").map(PathBuf::from);
    let slim = args.iter().any(|arg| arg == "--slim");
    let frame_limit = value("--frames").map(|s| {
        s.parse::<usize>()
            .expect("--frames needs a positive integer")
    });
    let report = value("--report").map(PathBuf::from);
    let paused = args.iter().any(|a| a == "--paused");
    gpui_platform::application().run(move |cx: &mut App| {
        let paused = paused || cx.reduce_motion();
        cx.bind_keys([
            gpui::KeyBinding::new("space", TogglePlayback, Some("GpuGallery")),
            gpui::KeyBinding::new("r", ResetCamera, Some("GpuGallery")),
        ]);
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(70.), px(60.)),
                        size(px(1280.), px(860.)),
                    ))),
                    window_min_size: Some(size(px(1100.), px(740.))),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("GPUI GPU Gallery".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        Gallery {
                            gpu: Rc::new(RefCell::new(GpuState {
                                renderer: None,
                                panels: [None, None, None],
                                error: None,
                                skin_path,
                                slim,
                                time: 2.4,
                                camera: [-0.48, -0.16, 1., 1.],
                                capture,
                                captured: [false; 3],
                                frame_cpu_ms: 0.,
                                samples: Vec::new(),
                                adapter: String::new(),
                            })),
                            last: Instant::now(),
                            paused,
                            focus,
                            dragging: false,
                            previous: point(px(0.), px(0.)),
                            frame_limit,
                            frames: 0,
                            report,
                        }
                    })
                },
            )
            .expect("opening GPU gallery");
        let _ = handle;
        cx.activate(true);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
}

#[cfg(test)]
#[gpui::test]
fn playback_and_camera_callbacks_work_outside_paint(cx: &mut gpui::TestAppContext) {
    let handle = cx.add_window(|_, cx| Gallery {
        gpu: Rc::new(RefCell::new(GpuState {
            renderer: None,
            panels: [None, None, None],
            error: None,
            time: 2.4,
            skin_path: None,
            slim: false,
            camera: [0.8, 0.3, 1.4, 1.],
            capture: None,
            captured: [false; 3],
            frame_cpu_ms: 0.,
            samples: Vec::new(),
            adapter: String::new(),
        })),
        last: Instant::now(),
        paused: false,
        focus: cx.focus_handle(),
        dragging: false,
        previous: point(px(0.), px(0.)),
        frame_limit: None,
        frames: 0,
        report: None,
    });
    // Input handlers run outside the rendered view stack. Calling the frame
    // scheduler here caused the reported pause-button panic on GPUI 0.1.17.
    handle
        .update(cx, |gallery, window, cx| {
            let before = gallery.gpu.borrow().time;
            gallery.toggle(&TogglePlayback, window, cx);
            assert!(gallery.paused);
            assert_eq!(gallery.gpu.borrow().time, before);
            gallery.reset(&ResetCamera, window, cx);
            assert_eq!(gallery.gpu.borrow().camera, [-0.48, -0.16, 1., 1.]);
            gallery.toggle(&TogglePlayback, window, cx);
            assert!(!gallery.paused);
        })
        .unwrap();
}
