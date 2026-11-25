use eframe::egui::{Pos2, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchTool {
    Unknown,
    Finger,
    Stylus,
    Eraser,
}

impl TouchTool {
    pub fn is_stylus(self) -> bool {
        matches!(self, Self::Stylus | Self::Eraser)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GesturePhase {
    Start,
    Move,
    End,
    Cancel,
}

#[derive(Clone, Copy, Debug)]
pub struct TouchSample {
    pub id: u64,
    pub pos: Pos2,
    pub force: Option<f32>,
    pub tool: TouchTool,
    pub phase: GesturePhase,
    pub time: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureMode {
    Idle,
    SingleFinger,
    PanZoom,
    Cooldown,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanZoomUpdate {
    pub center: Pos2,
    pub pan_delta: Vec2,
    pub zoom: f32,
    pub angle_delta: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tap {
    pub fingers: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureSettings {
    PanZoom,
    ZoomOnly,
    PanOnly,
}

impl Default for GestureSettings {
    fn default() -> Self {
        Self::PanZoom
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GestureAction {
    None,
    CancelFingerStroke,
    PanZoom(PanZoomUpdate),
    Tap(Tap),
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    pos: Pos2,
    start: Pos2,
}

#[derive(Clone, Debug)]
pub struct TouchGestureFsm {
    mode: GestureMode,
    fingers: Vec<Finger>,
    max_fingers: usize,
    start_time: f64,
    moved: bool,
    prev_center: Option<Pos2>,
    prev_dist: Option<f32>,
    prev_angle: Option<f32>,
    settings: GestureSettings,
    rotation_enabled: bool,
}

impl Default for TouchGestureFsm {
    fn default() -> Self {
        Self {
            mode: GestureMode::Idle,
            fingers: Vec::new(),
            max_fingers: 0,
            start_time: 0.0,
            moved: false,
            prev_center: None,
            prev_dist: None,
            prev_angle: None,
            settings: GestureSettings::PanZoom,
            rotation_enabled: false,
        }
    }
}

impl TouchGestureFsm {
    pub fn mode(&self) -> GestureMode {
        self.mode
    }

    pub fn finger_count(&self) -> usize {
        self.fingers.len()
    }

    pub fn set_settings(&mut self, settings: GestureSettings, rotation_enabled: bool) {
        self.settings = settings;
        self.rotation_enabled = rotation_enabled;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn centroid(&self) -> Pos2 {
        if self.fingers.is_empty() {
            return Pos2::ZERO;
        }
        let mut sum = Vec2::ZERO;
        for f in &self.fingers {
            sum += f.pos.to_vec2();
        }
        (sum / self.fingers.len() as f32).to_pos2()
    }

    fn remove_finger(&mut self, id: u64) {
        self.fingers.retain(|f| f.id != id);
    }

    fn update_finger(&mut self, id: u64, pos: Pos2) {
        if let Some(f) = self.fingers.iter_mut().find(|f| f.id == id) {
            f.pos = pos;
        } else {
            self.fingers.push(Finger { id, pos, start: pos });
        }
    }

    fn finish_if_idle(&mut self, now: f64) -> GestureAction {
        if self.fingers.is_empty() {
            let was_cancelled = self.mode == GestureMode::Idle;
            let tap = if !was_cancelled
                && !self.moved
                && self.prev_center.is_none()
                && now - self.start_time < 0.35
                && matches!(self.max_fingers, 2 | 3)
            {
                Some(GestureAction::Tap(Tap { fingers: self.max_fingers }))
            } else {
                None
            };
            self.reset();
            return tap.unwrap_or(GestureAction::None);
        }
        GestureAction::None
    }

    pub fn handle(&mut self, s: TouchSample) -> GestureAction {
        if s.tool.is_stylus() {
            return GestureAction::None;
        }

        match s.phase {
            GesturePhase::Start => {
                if self.mode == GestureMode::Cooldown {
                    self.fingers.push(Finger { id: s.id, pos: s.pos, start: s.pos });
                    self.max_fingers = self.max_fingers.max(self.fingers.len());
                    return GestureAction::None;
                }
                if self.fingers.is_empty() {
                    self.mode = GestureMode::SingleFinger;
                    self.start_time = s.time;
                    self.moved = false;
                    self.max_fingers = 0;
                    self.prev_center = None;
                    self.prev_dist = None;
                    self.prev_angle = None;
                }
                self.fingers.retain(|f| f.id != s.id);
                self.fingers.push(Finger { id: s.id, pos: s.pos, start: s.pos });
                self.max_fingers = self.max_fingers.max(self.fingers.len());
                if self.fingers.len() >= 2 && self.mode == GestureMode::SingleFinger {
                    self.mode = GestureMode::PanZoom;
                    // На второй палец начинаем заново: лёгкий дрейф первого пальца до этого
                    // не должен превращать быстрый тап в жест.
                    self.start_time = s.time;
                    self.moved = false;
                    self.prev_center = None;
                    self.prev_dist = None;
                    self.prev_angle = None;
                    return GestureAction::CancelFingerStroke;
                }
                GestureAction::None
            }
            GesturePhase::Move => {
                self.update_finger(s.id, s.pos);
                if let Some(f) = self.fingers.iter().find(|f| f.id == s.id) {
                    if (f.pos - f.start).length() > 12.0 {
                        self.moved = true;
                    }
                }
                if self.mode == GestureMode::PanZoom && self.fingers.len() >= 2 {
                    // Пока палец не вышел за slop, не инициализируем базу жеста: иначе
                    // микро-дрожание нижних MotionEvent съедало бы двухпальцевый тап (Undo).
                    if !self.moved {
                        return GestureAction::None;
                    }
                    let a = self.fingers[0].pos;
                    let b = self.fingers[1].pos;
                    let center = a + (b - a) / 2.0;
                    let dist = ((b - a).length() / 2.0).max(1.0);
                    let angle = (b - a).angle();
                    let mut out = PanZoomUpdate {
                        center,
                        pan_delta: Vec2::ZERO,
                        zoom: 1.0,
                        angle_delta: 0.0,
                    };
                    if let (Some(pc), Some(pd), Some(pa)) =
                        (self.prev_center, self.prev_dist, self.prev_angle)
                    {
                        if !matches!(self.settings, GestureSettings::ZoomOnly) {
                            out.pan_delta = center - pc;
                        }
                        if !matches!(self.settings, GestureSettings::PanOnly) {
                            out.zoom = (dist / pd).clamp(0.25, 4.0);
                        }
                        let mut da = angle - pa;
                        while da > std::f32::consts::PI {
                            da -= std::f32::consts::TAU;
                        }
                        while da < -std::f32::consts::PI {
                            da += std::f32::consts::TAU;
                        }
                        if self.rotation_enabled {
                            out.angle_delta = da;
                        }
                    }
                    self.prev_center = Some(center);
                    self.prev_dist = Some(dist);
                    self.prev_angle = Some(angle);
                    return GestureAction::PanZoom(out);
                }
                GestureAction::None
            }
            GesturePhase::End => {
                let mode_before = self.mode;
                let was_pan_zoom = mode_before == GestureMode::PanZoom && self.fingers.len() >= 2;
                self.remove_finger(s.id);
                if was_pan_zoom && self.fingers.len() < 2 {
                    // actual pan/zoom updates clear prev_*; a quick two-finger tap keeps them None
                    self.mode = GestureMode::Cooldown;
                }
                self.finish_if_idle(s.time)
            }
            GesturePhase::Cancel => {
                self.reset();
                GestureAction::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: u64, phase: GesturePhase, pos: Pos2, time: f64) -> TouchSample {
        TouchSample {
            id,
            pos,
            force: None,
            tool: TouchTool::Finger,
            phase,
            time,
        }
    }

    #[test]
    fn second_finger_cancels_stroke() {
        let mut fsm = TouchGestureFsm::default();
        assert_eq!(fsm.handle(sample(1, GesturePhase::Start, Pos2::new(0.0, 0.0), 0.0)), GestureAction::None);
        let act = fsm.handle(sample(2, GesturePhase::Start, Pos2::new(10.0, 0.0), 0.01));
        assert_eq!(act, GestureAction::CancelFingerStroke);
        assert_eq!(fsm.mode(), GestureMode::PanZoom);
    }

    #[test]
    fn cancel_resets_without_tap() {
        let mut fsm = TouchGestureFsm::default();
        fsm.handle(sample(1, GesturePhase::Start, Pos2::new(0.0, 0.0), 0.0));
        fsm.handle(sample(2, GesturePhase::Start, Pos2::new(10.0, 0.0), 0.01));
        let act = fsm.handle(sample(1, GesturePhase::Cancel, Pos2::new(0.0, 0.0), 0.02));
        assert_eq!(act, GestureAction::None);
        assert_eq!(fsm.mode(), GestureMode::Idle);
    }

    #[test]
    fn two_finger_tap_detected() {
        let mut fsm = TouchGestureFsm::default();
        fsm.handle(sample(1, GesturePhase::Start, Pos2::new(0.0, 0.0), 0.0));
        fsm.handle(sample(2, GesturePhase::Start, Pos2::new(10.0, 0.0), 0.01));
        fsm.handle(sample(1, GesturePhase::End, Pos2::new(0.0, 0.0), 0.02));
        let act = fsm.handle(sample(2, GesturePhase::End, Pos2::new(10.0, 0.0), 0.03));
        assert_eq!(act, GestureAction::Tap(Tap { fingers: 2 }));
    }

    #[test]
    fn small_jitter_does_not_block_two_finger_tap() {
        let mut fsm = TouchGestureFsm::default();
        fsm.handle(sample(1, GesturePhase::Start, Pos2::new(0.0, 0.0), 0.0));
        fsm.handle(sample(2, GesturePhase::Start, Pos2::new(10.0, 0.0), 0.01));
        // реальные касания всегда шлют Move с парой пикселей дрожания
        fsm.handle(sample(1, GesturePhase::Move, Pos2::new(1.0, 0.0), 0.02));
        fsm.handle(sample(2, GesturePhase::Move, Pos2::new(11.0, 0.0), 0.03));
        fsm.handle(sample(1, GesturePhase::End, Pos2::new(1.0, 0.0), 0.04));
        let act = fsm.handle(sample(2, GesturePhase::End, Pos2::new(11.0, 0.0), 0.05));
        assert_eq!(act, GestureAction::Tap(Tap { fingers: 2 }));
    }

    #[test]
    fn zoom_only_disables_pan() {
        let mut fsm = TouchGestureFsm::default();
        fsm.set_settings(GestureSettings::ZoomOnly, false);
        fsm.handle(sample(1, GesturePhase::Start, Pos2::new(0.0, 0.0), 0.0));
        fsm.handle(sample(2, GesturePhase::Start, Pos2::new(10.0, 0.0), 0.01));
        fsm.handle(sample(1, GesturePhase::Move, Pos2::new(0.0, 20.0), 0.02));
        fsm.handle(sample(2, GesturePhase::Move, Pos2::new(20.0, 20.0), 0.03)); // establishes baseline
        let act = fsm.handle(sample(2, GesturePhase::Move, Pos2::new(30.0, 20.0), 0.04));
        match act {
            GestureAction::PanZoom(u) => {
                assert!(u.pan_delta.length() < 0.001);
                assert!(u.zoom > 1.0);
            }
            _ => panic!("expected PanZoom"),
        }
    }

    #[test]
    fn cooldown_prevents_remaining_finger_from_becoming_single() {
        let mut fsm = TouchGestureFsm::default();
        fsm.handle(sample(1, GesturePhase::Start, Pos2::new(0.0, 0.0), 0.0));
        fsm.handle(sample(2, GesturePhase::Start, Pos2::new(10.0, 0.0), 0.01));
        fsm.handle(sample(1, GesturePhase::End, Pos2::new(0.0, 0.0), 0.02));
        assert_eq!(fsm.mode(), GestureMode::Cooldown);
        let act = fsm.handle(sample(2, GesturePhase::Move, Pos2::new(20.0, 0.0), 0.03));
        assert_eq!(act, GestureAction::None);
        assert_eq!(fsm.mode(), GestureMode::Cooldown);
    }
}
