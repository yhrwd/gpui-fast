//! Interactive material and motion studies. The flat study uses native GPUI
//! text, paths and quads; only the CNC material uses a custom fragment shader.
use super::motion::{Drag, Page, translate};
use super::{
    Gallery, Next, Previous, Primary, ShowCnc, ShowFlat, ShowScenes, TogglePlayback, muted,
};
use gpui::{
    Bounds, Context, Div, FontWeight, IntoElement, MouseButton, Pixels, Point, SharedString,
    Stateful, Window, canvas, div, fill, point, prelude::*, px, relative, rgb, size,
};
use std::time::Instant;

fn at(x: f32, y: f32, w: f32, h: f32) -> Div {
    div()
        .absolute()
        .left(relative(x / 900.))
        .top(relative(y / 500.))
        .w(relative(w / 900.))
        .h(relative(h / 500.))
}
fn legend(text: impl Into<SharedString>, x: f32, y: f32, w: f32) -> Div {
    at(x, y, w, 24.)
        .text_xs()
        .text_color(rgb(0xc4c9cd))
        .child(text.into())
}
fn control(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .focusable()
        .tab_index(0)
        .aria_label(label)
        .border_1()
        .border_color(gpui::transparent_black())
        .rounded_lg()
        .focus_visible(|s| s.border_color(rgb(0xf4b36c)))
}
fn rounded(window: &mut Window, bounds: Bounds<Pixels>, color: u32, radius: f32) {
    let mut q = fill(bounds, rgb(color));
    q.corner_radii = px(radius).into();
    window.paint_quad(q);
}
fn meter(value: f32, color: u32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            rounded(window, b, 0xe8e9ee, 4.);
            rounded(
                window,
                Bounds::new(
                    b.origin,
                    size(b.size.width * value.clamp(0., 1.), b.size.height),
                ),
                color,
                4.,
            );
        },
    )
    .w_full()
    .h(px(7.))
    .flex_shrink_0()
}
fn mix(a: u32, b: u32, value: f32) -> u32 {
    [0, 8, 16].into_iter().fold(0, |out, shift| {
        let a = ((a >> shift) & 255) as f32;
        let b = ((b >> shift) & 255) as f32;
        out | (((a + (b - a) * value.clamp(0., 1.)).round() as u32) << shift)
    })
}
fn switch(value: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            rounded(window, b, mix(0x898d9d, 0x4655da, value), 15.);
            rounded(
                window,
                Bounds::new(
                    b.origin + point(px(4. + value * 22.), px(4.)),
                    size(px(22.), px(22.)),
                ),
                0xffffff,
                11.,
            );
        },
    )
    .w(px(52.))
    .h(px(30.))
}

impl Gallery {
    pub(super) fn select_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.motion.page = page;
        self.motion.drag = None;
        self.dragging = false;
        self.last = Instant::now();
        self.animating = false;
        self.gpu.borrow_mut().samples.clear();
        cx.notify();
    }
    pub(super) fn activate(&mut self, cx: &mut Context<Self>) {
        self.motion.primary();
        if self.paused || cx.reduce_motion() {
            self.motion.snap();
        }
        cx.notify();
    }
    pub(super) fn root(&self, cx: &mut Context<Self>) -> Div {
        div()
            .track_focus(&self.focus)
            .key_context("GpuGallery")
            .on_action(cx.listener(Self::toggle))
            .on_action(cx.listener(Self::reset))
            .on_action(
                cx.listener(|this, _: &ShowScenes, _, cx| this.select_page(Page::Scenes, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowCnc, _, cx| this.select_page(Page::Cnc, cx)))
            .on_action(cx.listener(|this, _: &ShowFlat, _, cx| this.select_page(Page::Flat, cx)))
            .on_action(cx.listener(|this, _: &Primary, _, cx| {
                this.motion.primary();
                this.motion.snap();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Previous, _, cx| this.nudge(-1., cx)))
            .on_action(cx.listener(|this, _: &Next, _, cx| this.nudge(1., cx)))
    }
    fn nudge(&mut self, direction: f32, cx: &mut Context<Self>) {
        match self.motion.page {
            Page::Cnc => {
                self.motion.dial.target =
                    (self.motion.dial.target + direction * 0.02).clamp(0., 1.);
                self.motion.dial.snap();
            }
            Page::Flat => self.motion.set_tab(
                (self.motion.tab as i32 + direction as i32).rem_euclid(3) as usize,
                true,
            ),
            Page::Scenes => {}
        }
        cx.notify();
    }
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let mut sidebar = div()
            .w(px(174.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_3()
            .bg(rgb(0x0b1019))
            .text_color(rgb(0xe5eaf1))
            .border_r_1()
            .border_color(rgb(0x202937))
            .child(
                div()
                    .mb_4()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child("GPUI / LAB"),
            )
            .child(muted("MOTION STUDIES"));
        for (page, id, label) in [
            (Page::Scenes, "nav-scenes", "01  GPU gallery"),
            (Page::Cnc, "nav-cnc", "02  CNC console"),
            (Page::Flat, "nav-flat", "03  Flat motion"),
        ] {
            let selected = self.motion.page == page;
            sidebar = sidebar.child(
                control(id, label)
                    .w_full()
                    .px_3()
                    .py_3()
                    .bg(rgb(if selected { 0x222f31 } else { 0x0b1019 }))
                    .text_color(rgb(if selected { 0xc4e99a } else { 0xb0bbc9 }))
                    .on_click(cx.listener(move |this, _, _, cx| this.select_page(page, cx)))
                    .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            this.select_page(page, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .child(label),
            );
        }
        if self.motion.page == Page::Scenes {
            sidebar = sidebar
                .child(div().h(px(16.)))
                .child(muted("SKIN MODEL"))
                .child(
                    control("model-type", "Toggle classic and slim model")
                        .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| {
                            if e.keystroke.key == "enter" {
                                let mut gpu = this.gpu.borrow_mut();
                                gpu.slim = !gpu.slim;
                                gpu.panels = [None, None, None, None];
                                gpu.renderer = None;
                                drop(gpu);
                                cx.stop_propagation();
                                cx.notify();
                            }
                        }))
                        .p_2()
                        .text_color(rgb(0xc4e99a))
                        .child(if self.gpu.borrow().slim {
                            "Slim / 3px"
                        } else {
                            "Classic / 4px"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            let mut gpu = this.gpu.borrow_mut();
                            gpu.slim = !gpu.slim;
                            gpu.panels = [None, None, None, None];
                            gpu.renderer = None;
                            drop(gpu);
                            cx.notify();
                        })),
                );
        }
        sidebar
            .child(div().flex_1())
            .child(muted("1 / 2 / 3   Switch page"))
            .child(muted("SPACE   Play / Pause"))
            .child(div().h(px(12.)))
            .child(muted("FORK BASE"))
            .child(div().text_xs().child("GPUI Fast 0.1.17"))
    }
    pub(super) fn study_page(&self, cx: &mut Context<Self>) -> Div {
        let cnc = self.motion.page == Page::Cnc;
        let (eyebrow, title, description) = if cnc {
            (
                "02 / MATERIAL & MECHANISM",
                "Precision, you can feel.",
                "Turn the encoder. Move the feed. Feel the switch.",
            )
        } else {
            (
                "03 / INTERFACE & RESPONSE",
                "A little motion. A lot of clarity.",
                "Shared indicators, layered cards and feedback that follows your intent.",
            )
        };
        let error = self.gpu.borrow().error.clone();
        self.root(cx)
            .size_full()
            .flex()
            .font_family("Segoe UI")
            .text_sm()
            .bg(rgb(if cnc { 0x10151b } else { 0xf4f3f0 }))
            .text_color(rgb(if cnc { 0xe5eaf1 } else { 0x242839 }))
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(if cnc { 0xadb8c6 } else { 0x62677a }))
                                            .child(eyebrow),
                                    )
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(if cnc { 0xadb8c6 } else { 0x62677a }))
                                            .child(description),
                                    ),
                            )
                            .child(
                                control("playback", "Play or pause animations")
                                    .flex_shrink_0()
                                    .px_4()
                                    .py_3()
                                    .bg(rgb(if cnc { 0xc4e99a } else { 0x4655da }))
                                    .text_color(rgb(if cnc { 0x142019 } else { 0xffffff }))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle(&TogglePlayback, window, cx)
                                    }))
                                    .child(if self.paused {
                                        "Play / Space"
                                    } else {
                                        "Pause / Space"
                                    }),
                            ),
                    )
                    .child(if cnc {
                        self.cnc(cx).into_any_element()
                    } else {
                        self.flat(cx).into_any_element()
                    })
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .text_color(rgb(if cnc { 0xaeb9c7 } else { 0x62677a }))
                            .child(if cnc {
                                "Drag / scroll encoder   |   Click / drag feed   |   Enter: spindle"
                            } else {
                                "Click cards to inspect   |   Arrow keys: tabs   |   Enter: publish"
                            })
                            .child(if cx.reduce_motion() {
                                "Reduced motion"
                            } else if self.paused {
                                "Motion paused"
                            } else {
                                "Interactive study"
                            }),
                    )
                    .when_some(error, |this, e| {
                        this.child(div().text_color(rgb(0xd96357)).child(e))
                    }),
            )
    }
    pub(super) fn drag_control(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let dy = f32::from(self.motion.pointer.y - position.y);

        self.motion.pointer = position;
        match self.motion.drag {
            Some(Drag::Dial) => {
                self.motion.dial.target = (self.motion.dial.target + dy * 0.004).clamp(0., 1.)
            }
            Some(Drag::Feed) => {
                if let Some(bounds) = self.gpu.borrow().cnc_bounds {
                    let x = f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width)
                        * 900.;
                    self.motion.feed.target = ((x - 492.) / 320.).clamp(0., 1.);
                }
            }
            None => return,
        }
        if self.paused || cx.reduce_motion() {
            self.motion.snap();
        }
        cx.notify();
    }
    fn cnc(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let power = self.motion.power.target > 0.5;
        let speed = (self.motion.dial.value * 24000.) as u32;
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .rounded_xl()
            .overflow_hidden()
            .on_mouse_move(cx.listener(|this, e: &gpui::MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) {
                    this.drag_control(e.position, cx);
                } else {
                    this.motion.drag = None;
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.motion.drag = None;
                    this.motion.press.target = 0.;
                    if this.paused {
                        this.motion.press.snap();
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.motion.drag = None;
                    this.motion.press.target = 0.;
                    if this.paused {
                        this.motion.press.snap();
                    }
                    cx.notify();
                }),
            )
            .child(self.viewport(3))
            .child(
                at(56., 28., 560., 34.)
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child("FORM / 01"),
            )
            .child(
                at(660., 35., 178., 24.)
                    .text_xs()
                    .text_color(rgb(0xc4c9cd))
                    .text_right()
                    .child("PRECISION INSTRUMENTS"),
            )
            .child(legend("SPINDLE SPEED", 102., 101., 240.))
            .child(
                at(149., 373., 210., 36.)
                    .text_center()
                    .text_xl()
                    .font_family("Consolas")
                    .child(format!("{:05}  RPM", speed)),
            )
            .child(
                at(143., 408., 218., 24.)
                    .text_center()
                    .text_xs()
                    .text_color(rgb(0xc4c9cd))
                    .child("CONTINUOUS / 0 — 24,000"),
            )
            .child(legend("ACTUAL / RPM", 496., 113., 220.))
            .child(
                at(494., 139., 280., 62.)
                    .font_family("Consolas")
                    .text_size(px(42.))
                    .text_color(rgb(if power { 0xb8eab5 } else { 0x70867d }))
                    .child(format!(
                        "{:05}",
                        (speed as f32 * self.motion.power.value) as u32
                    )),
            )
            .child(legend(
                if power {
                    "SPINDLE ENABLED"
                } else {
                    "SPINDLE DISABLED"
                },
                496.,
                197.,
                260.,
            ))
            .child(legend("FEED OVERRIDE", 492., 277., 220.))
            .child(
                at(711., 275., 105., 24.)
                    .text_right()
                    .font_family("Consolas")
                    .child(format!("{:03}%", (self.motion.feed.value * 150.) as u32)),
            )
            .child(legend("0", 488., 342., 40.))
            .child(
                at(778., 342., 44., 22.)
                    .text_right()
                    .text_xs()
                    .text_color(rgb(0xc4c9cd))
                    .child("150"),
            )
            .child(legend("POWER", 484., 361., 90.))
            .child(
                at(634., 389., 190., 40.)
                    .text_center()
                    .font_weight(FontWeight::BOLD)
                    .child(translate(
                        div().child("HOME AXIS"),
                        0.,
                        self.motion.press.value
                            * self
                                .gpu
                                .borrow()
                                .cnc_bounds
                                .map_or(3., |b| f32::from(b.size.height) * 2.8 / 500.),
                    )),
            )
            .child(legend(
                "SERIAL 0017   /   AL–6061   /   MACHINED IN PIXELS",
                56.,
                449.,
                660.,
            ))
            .child(
                at(123., 112., 257., 255.).child(
                    control("encoder", "Spindle speed: drag vertically or scroll")
                        .size_full()
                        .cursor_grab()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, e: &gpui::MouseDownEvent, _, cx| {
                                this.motion.drag = Some(Drag::Dial);
                                this.motion.pointer = e.position;
                                cx.notify();
                            }),
                        )
                        .on_scroll(cx.listener(|this, e: &gpui::ScrollEvent, _, cx| {
                            let dy = f32::from(e.delta.pixel_delta(px(20.)).y);
                            this.motion.dial.target =
                                (this.motion.dial.target + dy * 0.001).clamp(0., 1.);
                            if this.paused || cx.reduce_motion() {
                                this.motion.snap();
                            }
                            cx.stop_propagation();
                            cx.notify();
                        })),
                ),
            )
            .child(
                at(478., 295., 354., 44.).child(
                    control("feed", "Feed override: click or drag")
                        .size_full()
                        .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| {
                            let direction = match e.keystroke.key.as_str() {
                                "left" => -0.02,
                                "right" => 0.02,
                                _ => return,
                            };
                            this.motion.feed.target =
                                (this.motion.feed.target + direction).clamp(0., 1.);
                            this.motion.feed.snap();
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .cursor_grab()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, e: &gpui::MouseDownEvent, _, cx| {
                                this.motion.drag = Some(Drag::Feed);
                                this.motion.pointer = e.position;
                                this.drag_control(e.position, cx);
                            }),
                        ),
                ),
            )
            .child(
                at(474., 377., 112., 60.).child(
                    control("spindle", "Toggle spindle power")
                        .size_full()
                        .on_click(cx.listener(|this, _, _, cx| this.activate(cx))),
                ),
            )
            .child(
                at(627., 375., 205., 63.).child(
                    control("home-axis", "Home axis: hold to depress")
                        .size_full()
                        .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| {
                            if e.keystroke.key == "enter" {
                                this.motion.dial.target = 0.5;
                                this.motion.feed.target = 0.5;
                                this.motion.snap();
                                cx.stop_propagation();
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.motion.press.target = 1.;
                                if this.paused || cx.reduce_motion() {
                                    this.motion.press.snap();
                                }
                                cx.notify();
                            }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.motion.dial.target = 0.5;
                            this.motion.feed.target = 0.5;
                            if this.paused || cx.reduce_motion() {
                                this.motion.snap();
                            }
                            cx.notify();
                        })),
                ),
            )
    }
    fn flat(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.motion.tab;
        let indicator = self.motion.indicator.value;
        let reveal = self.motion.reveal.value;
        let selected = self.motion.selection;
        let titles = [
            ["Aurora workspace", "Interface library", "Release notes"],
            [
                "Research & exploration",
                "Design review",
                "Production handoff",
            ],
            ["Active sessions", "Response quality", "Completed tasks"],
        ][tab];
        let subtitles = [
            [
                "A calm place to make things.",
                "24 components, one consistent language.",
                "The finishing touches for version 2.4.",
            ],
            [
                "Ideas become a clear direction.",
                "A little feedback goes a long way.",
                "Everything ready for the next step.",
            ],
            [
                "1,284 this week / up 18%",
                "98.6% positive / up 2.4%",
                "36 shipped / 4 in progress",
            ],
        ][tab];
        let colors = [0xe7eafd, 0xdff2eb, 0xffedd8];
        let running = self.motion.job.is_some();
        let progress = self.motion.progress.value;
        let toast = self.motion.toast.value;
        div().id("flat-workspace").relative().flex_1().min_h_0().overflow_y_scroll().flex().gap_4()
            .child(div().flex_1().min_w_0().min_h(px(500.)).flex().flex_col().gap_3()
                .child(div().flex().justify_between().items_center().mb_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Your workspace"))
                    .child(div().text_xs().text_color(rgb(0x62677a)).child("3 collections")))
                .child(div().relative().h(px(48.)).flex_shrink_0().rounded_lg().bg(rgb(0xe9e9e5))
                    .child(canvas(|_,_,_|{},move |b,_,window,_| {
                        let width=(f32::from(b.size.width)-8.)/3.;
                        rounded(window,Bounds::new(b.origin+point(px(4.+indicator*width),px(4.)),size(px(width),px(40.))),0xffffff,8.);
                    }).absolute().size_full())
                    .child(div().size_full().flex().children(["Overview","Timeline","Insights"].into_iter().enumerate().map(|(i,label)| {
                        control(["tab-overview","tab-timeline","tab-insights"][i],label).flex_1().h_full().flex().items_center().justify_center()
                            .text_color(rgb(if tab==i{0x343cba}else{0x62677a})).font_weight(FontWeight::SEMIBOLD)
                            .on_click(cx.listener(move |this,_,_,cx|{
                                this.motion.set_tab(i,this.paused||cx.reduce_motion());cx.notify();
                            })).on_key_down(cx.listener(move |this,e: &gpui::KeyDownEvent,_,cx| {
                                if e.keystroke.key=="enter" {this.motion.set_tab(i,true);cx.stop_propagation();cx.notify();}
                            })).child(label)
                    }))))
                .children((0..3).map(|i| {
                    let row=self.motion.rows[i].value;
                    translate(control(["card-one","card-two","card-three"][i],titles[i])
                        .h(px(88.)).w_full().flex_shrink_0().px_4().flex().gap_3().items_center()
                        .bg(rgb(0xffffff)).border_color(rgb(if selected==i && self.motion.reveal.target>0.{0x929cef}else{0xe2e3e8}))
                        .opacity(0.55+row*0.45).hover(|s|s.bg(rgb(0xfafbff)))
                        .on_click(cx.listener(move |this,_,_,cx|{
                            this.motion.reveal.target=if this.motion.selection==i {1.-this.motion.reveal.target}else{1.};
                            this.motion.selection=i;if this.paused||cx.reduce_motion(){this.motion.snap();}cx.notify();
                        }))
                        .on_key_down(cx.listener(move |this,e: &gpui::KeyDownEvent,_,cx|{
                            if e.keystroke.key=="enter"{
                                this.motion.selection=i;this.motion.reveal.target=1.-this.motion.reveal.target;
                                this.motion.reveal.snap();cx.stop_propagation();cx.notify();
                            }
                        }))
                        .child(div().size(px(46.)).rounded_lg().bg(rgb(colors[i])).text_color(rgb(0x404b6a))
                            .flex().items_center().justify_center().font_weight(FontWeight::BOLD).child(["Au","Ui","Re"][i]))
                        .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(titles[i]))
                            .child(div().text_xs().text_color(rgb(0x62677a)).child(subtitles[i])))
                        .child(div().text_lg().text_color(rgb(0x7c8399)).child(if selected==i && self.motion.reveal.target>0.{"−"}else{"+"})),
                        (1.-row)*18.,0.)
                }))
                .child(div().flex_1())
                .child(div().flex().items_center().justify_between().p_4().rounded_lg().bg(rgb(0xe9ece8))
                    .child(div().flex().flex_col().gap_1().child("Keep everything in sync")
                        .child(div().text_xs().text_color(rgb(0x62677a)).child(if self.motion.enabled.target>0.5{"Automatic updates are on"}else{"Automatic updates are off"})))
                    .child(control("sync-toggle","Toggle automatic updates").p_1()
                        .on_key_down(cx.listener(|this,e: &gpui::KeyDownEvent,_,cx| {
                            if e.keystroke.key=="enter" {
                                this.motion.enabled.target=1.-this.motion.enabled.target;
                                this.motion.enabled.snap();cx.stop_propagation();cx.notify();
                            }
                        }))
                        .on_click(cx.listener(|this,_,_,cx| {
                            this.motion.enabled.target=1.-this.motion.enabled.target;
                            if this.paused||cx.reduce_motion(){this.motion.enabled.snap();}cx.notify();
                        })).child(switch(self.motion.enabled.value)))))
            .child(div().w(px(288.)).min_h(px(500.)).flex_shrink_0().flex().flex_col().gap_3()
                .child(div().relative().h(px(258.)).rounded_xl().overflow_hidden().bg(rgb(0xe8ebfb)).p_5()
                    .child(div().text_xs().text_color(rgb(0x4f5a9a)).child("COLLECTION / DETAILS"))
                    .child(translate(div().mt_5().flex().flex_col().gap_3().opacity((1.-reveal*2.).max(0.)).when(reveal>=0.5,|this|this.invisible())
                        .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child("Room for a
closer look."))
                        .child(div().text_color(rgb(0x56607e)).child("Select a collection to reveal its details. Select it again to tuck them away.")),0.,-reveal*16.))
                    .child(div().absolute().inset_0().pt(px(57.)).px_5().child(translate(
                        div().flex().flex_col().gap_3().opacity((reveal*2.-1.).max(0.)).when(reveal<=0.5,|this|this.invisible())
                            .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(titles[selected]))
                            .child(div().text_color(rgb(0x56607e)).child(subtitles[selected]))
                            .child(div().h(px(1.)).bg(rgb(0xc9cfea)))
                            .child(div().flex().justify_between().text_xs().child("Last updated").child("Just now"))
                            .child(div().flex().justify_between().text_xs().child("Status").child("Ready to share"))
                            .child(div().text_xs().text_color(rgb(0x4350aa)).child("Select the card again to close")),0.,(1.-reveal)*18.))))
                .child(div().flex_1().min_h_0().p_5().rounded_xl().bg(rgb(0xffffff)).border_1().border_color(rgb(0xe2e3e8)).flex().flex_col().gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Ready when you are."))
                    .child(div().text_xs().text_color(rgb(0x62677a)).child("A local publishing demo. Try canceling and starting again."))
                    .child(meter(progress,0x4655da))
                    .child(div().flex().justify_between().text_xs().text_color(rgb(0x62677a))
                        .child(if running{if progress<0.33{"Preparing files"}else if progress<0.7{"Checking details"}else{"Finishing up"}}else if progress>0.99{"Everything is published"}else{"Waiting for your cue"})
                        .child(format!("{}%",(progress*100.) as u32)))
                    .child(div().flex_1())
                    .child(control("publish","Publish or cancel the local demo").w_full().py_3().flex().items_center().justify_center()
                        .bg(rgb(if running{0xe9ebf8}else{0x4655da})).text_color(rgb(if running{0x3542ad}else{0xffffff}))
                        .on_click(cx.listener(|this,_,_,cx|this.activate(cx))).child(if running{"Cancel publish"}else{"Publish changes"}))))
            .when(toast>0.001,|this|this.child(div().absolute().bottom(px(16.)).left(px(24.)).right(px(24.))
                .child(translate(div().id("publish-toast").w_full().flex().justify_between().items_center().gap_3().px_5().py_4()
                    .rounded_xl().bg(rgb(0x233b33)).text_color(rgb(0xe3f6ec)).opacity(toast)
                    .child(div().flex().flex_col().gap_1().child("All set. Your changes are published.")
                        .child(div().text_xs().text_color(rgb(0xc0ddcf)).child("Local demo completed successfully.")))
                    .child(control("dismiss-toast","Dismiss notification")
                        .on_key_down(cx.listener(|this,e: &gpui::KeyDownEvent,_,cx| {
                            if e.keystroke.key=="enter" {
                                this.motion.toast.target=0.;this.motion.toast.snap();cx.stop_propagation();cx.notify();
                            }
                        })).px_3().py_2().child("Dismiss")
                        .on_click(cx.listener(|this,_,_,cx|{
                            this.motion.toast.target=0.;if this.paused||cx.reduce_motion(){this.motion.snap();}cx.notify();
                        }))),0.,(1.-toast)*20.))))
    }
}
