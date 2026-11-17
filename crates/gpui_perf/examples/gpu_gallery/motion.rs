//! Frame-rate independent, interruptible responses used by both UI studies.
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Point, Window, point, px,
};

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub value: f32,
    pub target: f32,
    velocity: f32,
}
impl Spring {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            velocity: 0.,
        }
    }
    pub fn snap(&mut self) {
        self.value = self.target;
        self.velocity = 0.;
    }
    // Exact critically damped solution. Retargeting preserves position AND velocity;
    // unlike Euler integration, the result does not depend on display refresh rate.
    pub fn step(&mut self, dt: f32, frequency: f32) -> bool {
        let offset = self.value - self.target;
        let c = self.velocity + frequency * offset;
        let decay = (-frequency * dt).exp();
        self.value = self.target + (offset + c * dt) * decay;
        self.velocity = (self.velocity - frequency * c * dt) * decay;
        if (self.value - self.target).abs() < 0.0005 && self.velocity.abs() < 0.005 {
            self.snap();
            false
        } else {
            true
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Scenes,
    Cnc,
    Flat,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drag {
    Dial,
    Feed,
}

pub struct Motion {
    pub page: Page,
    pub dial: Spring,
    pub feed: Spring,
    pub power: Spring,
    pub press: Spring,
    pub drag: Option<Drag>,
    pub pointer: Point<Pixels>,
    pub tab: usize,
    pub indicator: Spring,
    pub reveal: Spring,
    pub selection: usize,
    pub enabled: Spring,
    pub toast: Spring,
    pub progress: Spring,
    pub job: Option<f32>,
    pub rows: [Spring; 3],
    row_age: f32,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            page: Page::Scenes,
            dial: Spring::new(0.58),
            feed: Spring::new(0.64),
            power: Spring::new(1.),
            press: Spring::new(0.),
            drag: None,
            pointer: point(px(0.), px(0.)),
            tab: 0,
            indicator: Spring::new(0.),
            reveal: Spring::new(0.),
            selection: 0,
            enabled: Spring::new(1.),
            toast: Spring::new(0.),
            progress: Spring::new(0.),
            job: None,
            rows: [Spring::new(1.); 3],
            row_age: 1.,
        }
    }
}
impl Motion {
    pub fn new(page: Page) -> Self {
        Self {
            page,
            ..Default::default()
        }
    }
    pub fn set_tab(&mut self, tab: usize, instant: bool) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        self.indicator.target = tab as f32;
        self.reveal.target = 0.;
        self.row_age = 0.;
        // Retarget a short outgoing response, then stagger each return. Keep
        // the current position and velocity when a second selection interrupts.
        for row in &mut self.rows {
            row.target = if instant { 1. } else { 0. };
        }
        if instant {
            self.snap();
        }
    }
    pub fn primary(&mut self) {
        match self.page {
            Page::Cnc => {
                self.power.target = 1. - self.power.target;
            }
            Page::Flat => {
                if self.job.is_some() {
                    self.job = None;
                    self.progress.target = 0.;
                } else {
                    self.progress.value = 0.;
                    self.progress.target = 0.;
                    self.job = Some(0.);
                    self.toast.target = 0.;
                }
            }
            Page::Scenes => {}
        }
    }
    pub fn snap(&mut self) {
        for s in [
            &mut self.dial,
            &mut self.feed,
            &mut self.power,
            &mut self.press,
            &mut self.indicator,
            &mut self.reveal,
            &mut self.enabled,
            &mut self.toast,
            &mut self.progress,
        ] {
            s.snap();
        }
        for row in &mut self.rows {
            row.target = 1.;
            row.snap();
        }
    }
    pub fn tick(&mut self, dt: f32, reduced: bool) -> bool {
        if self.page == Page::Scenes {
            return false;
        }
        if self.page == Page::Cnc {
            if reduced {
                self.snap();
                return false;
            }
            return self.dial.step(dt, 32.)
                | self.feed.step(dt, 32.)
                | self.power.step(dt, 32.)
                | self
                    .press
                    .step(dt, if self.press.target > 0. { 42. } else { 60. });
        }
        self.row_age += dt;
        if let Some(age) = &mut self.job {
            *age += dt;
            self.progress.target = (*age / 3.2).min(1.);
            if *age >= 3.2 {
                self.job = None;
                self.toast.target = 1.;
            }
        }
        if reduced {
            self.snap();
            return self.job.is_some();
        }
        let mut moving = self.indicator.step(dt, 34.)
            | self.reveal.step(dt, 34.)
            | self.enabled.step(dt, 36.)
            | self.toast.step(dt, 34.)
            | self.progress.step(dt, 36.);
        for (index, row) in self.rows.iter_mut().enumerate() {
            if self.row_age >= 0.045 + index as f32 * 0.035 {
                row.target = 1.;
            } else {
                moving = true;
            }
            moving |= row.step(dt, 38.);
        }
        moving || self.job.is_some()
    }
}

/// Translate during prepaint, after Taffy layout. Child hitboxes and text follow
/// the same offset; no animated width/height or repeated layout interpolation.
pub struct Translate {
    child: AnyElement,
    offset: Point<Pixels>,
}
pub fn translate(child: impl IntoElement, x: f32, y: f32) -> Translate {
    Translate {
        child: child.into_any_element(),
        offset: point(px(x), px(y)),
    }
}
impl Element for Translate {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_element_offset(self.offset, |window| self.child.prepaint(window, cx));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_is_independent_of_frame_rate() {
        let mut a = Spring::new(0.);
        a.target = 1.;
        let mut b = a;
        for _ in 0..30 {
            a.step(1. / 60., 32.);
        }
        for _ in 0..72 {
            b.step(1. / 144., 32.);
        }
        assert!((a.value - b.value).abs() < 0.00001);
        assert_eq!(a.value, 1.);
    }
    #[test]
    fn reversal_preserves_velocity_and_eventually_stops() {
        let mut s = Spring::new(0.);
        s.target = 1.;
        s.step(0.035, 32.);
        let (value, velocity) = (s.value, s.velocity);
        s.target = 0.;
        assert_eq!((s.value, s.velocity), (value, velocity));
        s.step(0.001, 32.);
        assert!(s.value > value);
        for _ in 0..80 {
            s.step(1. / 60., 32.);
        }
        assert!(!s.step(1. / 60., 32.));
        assert_eq!(s.value, 0.);
    }
    #[test]
    fn switching_tabs_mid_transition_does_not_teleport_rows() {
        let mut m = Motion::new(Page::Flat);
        m.set_tab(1, false);
        m.tick(0.016, false);
        let before = m.rows.map(|s| (s.value, s.velocity));
        m.set_tab(2, false);
        assert_eq!(m.rows.map(|s| (s.value, s.velocity)), before);
        for _ in 0..80 {
            m.tick(1. / 60., false);
        }
        assert!(!m.tick(1. / 60., false));
        assert!(m.rows.iter().all(|s| s.value == 1.));
    }
    #[test]
    fn jobs_and_reduced_motion_settle() {
        let mut m = Motion {
            page: Page::Flat,
            ..Default::default()
        };
        m.primary();
        for _ in 0..210 {
            m.tick(1. / 60., true);
        }
        assert!(m.job.is_none());
        assert_eq!(m.progress.value, 1.);
        assert_eq!(m.toast.value, 1.);
        assert!(!m.tick(1. / 60., false));
        m.set_tab(2, true);
        assert_eq!(m.indicator.value, 2.);
    }
}
