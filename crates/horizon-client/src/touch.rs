//! Touch controls for play, after Bedrock Edition's as the TouchController mod rebuilds them for
//! Java Edition (its default preset): a D-pad with sneak in its centre, a jump button that turns
//! into ascend and descend while flying or swimming, chat and pause at the top, an inventory
//! button beside the hotbar. A finger on the world looks around; a tap uses (or attacks the
//! entity) where it lands, a hold mines or uses there. Positions are GUI pixels.

/// A finger held this long without moving holds (`viewHoldDetectTicks`, 5 ticks).
pub const HOLD_SECONDS: f64 = 0.25;
/// A hotbar slot held this long drops the selected stack (40 ticks).
pub const DROP_SECONDS: f64 = 2.0;
/// A finger that moved this fraction of the window's width no longer taps or holds.
const MOVE_FRACTION: f32 = 0.02;
/// Degrees turned per window width a finger moves (`viewMovementSensitivity`).
const LOOK_DEGREES: f32 = 495.0;
/// The touch ring's radius; its outline is 2 more.
pub const RING_RADIUS: f32 = 36.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Forward,
    Back,
    Left,
    Right,
    ForwardLeft,
    ForwardRight,
    BackLeft,
    BackRight,
    /// The D-pad's centre: a tap toggles sneaking.
    Sneak,
    Jump,
    Ascend,
    Descend,
    Chat,
    Pause,
    Inventory,
}

impl Control {
    /// Held while a finger is on it, and a finger slides from one to another.
    fn swipe(self) -> bool {
        !matches!(
            self,
            Control::Sneak | Control::Chat | Control::Pause | Control::Inventory
        )
    }
}

/// What the game acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Sneak, chat and pause on touch, the inventory on release.
    Press(Control),
    /// A tap selects the hotbar slot.
    Slot(usize),
    /// A hotbar slot held: drop the selected stack.
    Drop,
    /// A short touch on the world, still.
    Tap,
    /// A touch on the world held still.
    Hold,
    /// The held touch ended.
    Release,
}

/// Where the controls are, for a GUI of `width` by `height`: the geometry of TouchController's
/// `new` texture set (Bedrock's current buttons), 22 pixel textures drawn at twice their size.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    /// Flying or swimming: ascend and descend replace jump.
    pub ascend: bool,
    /// The forward diagonals show while forward, left or right is held.
    pub forward_diagonals: bool,
    /// The backward ones while back, left or right is.
    pub backward_diagonals: bool,
}

type Rect = [f32; 4];

fn inside(r: Rect, [x, y]: [f32; 2]) -> bool {
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

/// A D-pad button; its neighbours overlap it by 2 (the `new` set's padding of -1).
const BUTTON: f32 = 44.0;
const STEP: f32 = BUTTON - 2.0;

impl Layout {
    /// The D-pad's top left: 12 from the left, 16 from the bottom, 3 buttons square.
    fn dpad(&self) -> [f32; 2] {
        [12.0, self.height - 16.0 - (2.0 * STEP + BUTTON)]
    }

    /// The controls shown, with where each is drawn and touched.
    pub fn controls(&self) -> Vec<(Control, Rect)> {
        let [dx, dy] = self.dpad();
        let cell = |i: f32, j: f32| [dx + i * STEP, dy + j * STEP, BUTTON, BUTTON];
        let (w, h) = (self.width, self.height);
        let mut controls = vec![
            (Control::Forward, cell(1.0, 0.0)),
            (Control::Left, cell(0.0, 1.0)),
            (Control::Right, cell(2.0, 1.0)),
            (Control::Back, cell(1.0, 2.0)),
            (Control::Sneak, cell(1.0, 1.0)),
        ];
        if self.forward_diagonals {
            controls.push((Control::ForwardLeft, cell(0.0, 0.0)));
            controls.push((Control::ForwardRight, cell(2.0, 0.0)));
        }
        if self.backward_diagonals {
            controls.push((Control::BackLeft, cell(0.0, 2.0)));
            controls.push((Control::BackRight, cell(2.0, 2.0)));
        }
        // 42 from the right; 68 from the bottom, descend 18.
        let right = w - 42.0 - BUTTON;
        if self.ascend {
            controls.push((Control::Ascend, [right, h - 68.0 - BUTTON, BUTTON, BUTTON]));
            controls.push((Control::Descend, [right, h - 18.0 - BUTTON, BUTTON, BUTTON]));
        } else {
            controls.push((Control::Jump, [right, h - 68.0 - BUTTON, BUTTON, BUTTON]));
        }
        controls.push((Control::Chat, [(w / 2.0).floor() - 18.0, 0.0, 18.0, 18.0]));
        controls.push((Control::Pause, [(w / 2.0).floor(), 0.0, 18.0, 18.0]));
        controls.push((
            Control::Inventory,
            [(w / 2.0).floor() + 93.0, h - 22.0, 22.0, 22.0],
        ));
        controls
    }

    pub fn control_at(&self, at: [f32; 2]) -> Option<Control> {
        self.controls()
            .into_iter()
            .find(|&(_, r)| inside(r, at))
            .map(|(c, _)| c)
    }

    /// The hotbar slot under `at` (`Gui.renderHotbar`'s 182 by 22, centred at the bottom).
    pub fn slot_at(&self, [x, y]: [f32; 2]) -> Option<usize> {
        let left = (self.width / 2.0).floor() - 91.0;
        if y < self.height - 22.0 || y >= self.height || x < left + 1.0 || x >= left + 181.0 {
            return None;
        }
        Some((((x - left - 1.0) / 20.0) as usize).min(8))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Still,
    Holding,
    /// Moved, or held over nothing: only looks.
    Done,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Swipe,
    /// Acts when it lifts (the inventory button).
    OnRelease(Control),
    Slot {
        slot: usize,
        since: f64,
    },
    Look {
        start: [f32; 2],
        since: f64,
        moved: bool,
        phase: Phase,
    },
    /// A second finger on the world, or one whose action is done.
    Ignored,
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    at: [f32; 2],
    kind: Kind,
}

#[derive(Debug, Default)]
pub struct TouchControls {
    fingers: Vec<Finger>,
    /// Degrees to turn since the last `take_look`: yaw, pitch.
    look: [f32; 2],
    forward_diagonals: bool,
    backward_diagonals: bool,
    ascend: bool,
}

impl TouchControls {
    pub fn layout(&self, width: f32, height: f32) -> Layout {
        Layout {
            width,
            height,
            ascend: self.ascend,
            forward_diagonals: self.forward_diagonals,
            backward_diagonals: self.backward_diagonals,
        }
    }

    /// Flying or swimming: ascend and descend take jump's place.
    pub fn set_ascend(&mut self, ascend: bool) {
        self.ascend = ascend;
    }

    pub fn down(&mut self, layout: &Layout, id: u64, at: [f32; 2], now: f64) -> Vec<Event> {
        let mut events = Vec::new();
        let kind = if let Some(control) = layout.control_at(at) {
            match control {
                Control::Inventory => Kind::OnRelease(control),
                Control::Chat | Control::Pause => {
                    events.push(Event::Press(control));
                    Kind::Ignored
                }
                // It slides onto the D-pad's arms like any of them.
                Control::Sneak => {
                    events.push(Event::Press(control));
                    Kind::Swipe
                }
                _ => Kind::Swipe,
            }
        } else if let Some(slot) = layout.slot_at(at) {
            Kind::Slot { slot, since: now }
        } else if self.look_finger().is_none() {
            Kind::Look {
                start: at,
                since: now,
                moved: false,
                phase: Phase::Still,
            }
        } else {
            Kind::Ignored
        };
        self.fingers.retain(|f| f.id != id);
        self.fingers.push(Finger { id, at, kind });
        self.update_diagonals(layout);
        events
    }

    pub fn moved(&mut self, layout: &Layout, id: u64, at: [f32; 2]) {
        let Some(finger) = self.fingers.iter_mut().find(|f| f.id == id) else {
            return;
        };
        if let Kind::Look { start, moved, .. } = &mut finger.kind {
            let [dx, dy] = [at[0] - finger.at[0], at[1] - finger.at[1]];
            self.look[0] += dx / layout.width * LOOK_DEGREES;
            self.look[1] += dy / layout.width * LOOK_DEGREES;
            let [sx, sy] = [at[0] - start[0], at[1] - start[1]];
            let limit = MOVE_FRACTION * layout.width;
            *moved |= sx * sx + sy * sy > limit * limit;
        }
        finger.at = at;
        self.update_diagonals(layout);
    }

    /// A finger lifted (`cancelled`: the system took it, so it does nothing but end).
    pub fn up(&mut self, layout: &Layout, id: u64, now: f64, cancelled: bool) -> Vec<Event> {
        let Some(i) = self.fingers.iter().position(|f| f.id == id) else {
            return Vec::new();
        };
        let finger = self.fingers.remove(i);
        self.update_diagonals(layout);
        let event = match finger.kind {
            Kind::Look {
                phase: Phase::Holding,
                ..
            } => Some(Event::Release),
            _ if cancelled => None,
            Kind::Look {
                since,
                moved: false,
                phase: Phase::Still,
                ..
            } if now - since < HOLD_SECONDS => Some(Event::Tap),
            Kind::Slot { slot, .. } if layout.slot_at(finger.at) == Some(slot) => {
                Some(Event::Slot(slot))
            }
            Kind::OnRelease(control) => Some(Event::Press(control)),
            _ => None,
        };
        event.into_iter().collect()
    }

    /// Once a frame: the holds that started.
    pub fn frame(&mut self, now: f64) -> Vec<Event> {
        let mut events = Vec::new();
        for finger in &mut self.fingers {
            match &mut finger.kind {
                Kind::Look {
                    since,
                    moved: false,
                    phase: phase @ Phase::Still,
                    ..
                } if now - *since >= HOLD_SECONDS => {
                    *phase = Phase::Holding;
                    events.push(Event::Hold);
                }
                Kind::Slot { since, .. } if now - *since >= DROP_SECONDS => {
                    finger.kind = Kind::Ignored;
                    events.push(Event::Drop);
                }
                _ => {}
            }
        }
        events
    }

    /// The hold had nothing to act on: the finger only looks from now on.
    pub fn hold_unused(&mut self) {
        for finger in &mut self.fingers {
            if let Kind::Look { phase, .. } = &mut finger.kind
                && *phase == Phase::Holding
            {
                *phase = Phase::Done;
            }
        }
    }

    /// A screen opened: every finger ends, and a hold is released.
    pub fn clear(&mut self) -> Vec<Event> {
        let holding = self.fingers.iter().any(|f| {
            matches!(
                f.kind,
                Kind::Look {
                    phase: Phase::Holding,
                    ..
                }
            )
        });
        self.fingers.clear();
        self.forward_diagonals = false;
        self.backward_diagonals = false;
        self.look = [0.0; 2];
        if holding {
            vec![Event::Release]
        } else {
            Vec::new()
        }
    }

    /// The degrees to turn since the last call: yaw, then pitch (down positive).
    pub fn take_look(&mut self) -> [f32; 2] {
        std::mem::take(&mut self.look)
    }

    /// The swipe controls fingers are on.
    pub fn held(&self, layout: &Layout) -> Vec<Control> {
        let mut held: Vec<Control> = self
            .fingers
            .iter()
            .filter(|f| matches!(f.kind, Kind::Swipe))
            .filter_map(|f| layout.control_at(f.at))
            .filter(|c| c.swipe())
            .collect();
        held.dedup();
        held
    }

    /// The D-pad as a stick: `[left, forward]`, each -1 to 1.
    pub fn move_vector(&self, layout: &Layout) -> Option<[f32; 2]> {
        let mut v = [0.0f32; 2];
        for control in self.held(layout) {
            let [l, f] = match control {
                Control::Forward => [0.0, 1.0],
                Control::Back => [0.0, -1.0],
                Control::Left => [1.0, 0.0],
                Control::Right => [-1.0, 0.0],
                Control::ForwardLeft => [1.0, 1.0],
                Control::ForwardRight => [-1.0, 1.0],
                Control::BackLeft => [1.0, -1.0],
                Control::BackRight => [-1.0, -1.0],
                _ => continue,
            };
            v = [v[0] + l, v[1] + f];
        }
        let v = v.map(|c| c.clamp(-1.0, 1.0));
        (v != [0.0, 0.0]).then_some(v)
    }

    fn look_finger(&self) -> Option<&Finger> {
        self.fingers
            .iter()
            .find(|f| matches!(f.kind, Kind::Look { .. }))
    }

    /// Where the finger on the world is: the touch ring, and what it aims at.
    pub fn aim(&self) -> Option<[f32; 2]> {
        self.look_finger().map(|f| f.at)
    }

    pub fn is_empty(&self) -> bool {
        self.fingers.is_empty()
    }

    fn update_diagonals(&mut self, layout: &Layout) {
        let layout = self.layout(layout.width, layout.height);
        let held = self.held(&layout);
        let any = |controls: &[Control]| held.iter().any(|c| controls.contains(c));
        use Control::*;
        self.forward_diagonals = any(&[Forward, ForwardLeft, ForwardRight, Left, Right]);
        self.backward_diagonals = any(&[Back, BackLeft, BackRight, Left, Right]);
    }
}

/// The unit direction through the screen point `at` (GUI pixels) of a view `yaw`/`pitch`
/// (degrees, as `Entity.getViewVector`) with a vertical field of view of `fov` degrees.
pub fn aim_vector(at: [f32; 2], size: [f32; 2], fov: f32, yaw: f32, pitch: f32) -> [f64; 3] {
    let half = (f64::from(fov) / 2.0).to_radians().tan();
    let aspect = f64::from(size[0]) / f64::from(size[1]);
    // Camera space: x left, y up, z forward (a view vector of yaw and pitch 0 is +z).
    let sx = (f64::from(at[0]) / f64::from(size[0])) * 2.0 - 1.0;
    let sy = 1.0 - (f64::from(at[1]) / f64::from(size[1])) * 2.0;
    let [cx, cy, cz] = [-sx * half * aspect, sy * half, 1.0];
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    // Pitch about x (positive looks down), then yaw about y.
    let (py, pz) = (cy * p.cos() - cz * p.sin(), cy * p.sin() + cz * p.cos());
    let (wx, wz) = (cx * y.cos() - pz * y.sin(), cx * y.sin() + pz * y.cos());
    let len = (wx * wx + py * py + wz * wz).sqrt();
    [wx / len, py / len, wz / len]
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 427.0;
    const H: f32 = 240.0;

    fn layout(touch: &TouchControls) -> Layout {
        touch.layout(W, H)
    }

    /// The middle of the D-pad's cell `i`, `j` (the centre is 1, 1).
    fn cell(i: f32, j: f32) -> [f32; 2] {
        [12.0 + i * 42.0 + 22.0, H - 144.0 + j * 42.0 + 22.0]
    }

    #[test]
    fn a_still_short_touch_on_the_world_taps() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        assert!(t.down(&l, 1, [200.0, 100.0], 0.0).is_empty());
        assert!(t.frame(0.1).is_empty());
        assert_eq!(t.up(&l, 1, 0.2, false), vec![Event::Tap]);
    }

    #[test]
    fn a_still_long_touch_holds_until_it_lifts() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        t.down(&l, 1, [200.0, 100.0], 0.0);
        assert_eq!(t.frame(0.25), vec![Event::Hold]);
        assert!(t.frame(0.5).is_empty());
        // Moving while holding keeps mining, and looks.
        t.moved(&l, 1, [260.0, 100.0]);
        assert!(t.take_look()[0] > 60.0);
        assert_eq!(t.up(&l, 1, 1.0, false), vec![Event::Release]);
    }

    #[test]
    fn a_drag_only_looks() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        t.down(&l, 1, [200.0, 100.0], 0.0);
        t.moved(&l, 1, [200.0 + W, 100.0]);
        assert_eq!(t.take_look(), [LOOK_DEGREES, 0.0]);
        assert!(t.frame(1.0).is_empty());
        assert!(t.up(&l, 1, 1.0, false).is_empty());
    }

    #[test]
    fn a_small_wobble_still_taps() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        t.down(&l, 1, [200.0, 100.0], 0.0);
        t.moved(&l, 1, [203.0, 101.0]);
        assert_eq!(t.up(&l, 1, 0.1, false), vec![Event::Tap]);
    }

    #[test]
    fn a_second_world_finger_is_ignored() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        t.down(&l, 1, [200.0, 100.0], 0.0);
        t.down(&l, 2, [300.0, 100.0], 0.0);
        t.moved(&l, 2, [350.0, 100.0]);
        assert_eq!(t.take_look(), [0.0, 0.0]);
        assert!(t.up(&l, 2, 0.1, false).is_empty());
        assert_eq!(t.aim(), Some([200.0, 100.0]));
    }

    #[test]
    fn the_dpad_moves_and_slides_between_arms() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        // The forward arm, at the D-pad's top centre.
        t.down(&l, 1, cell(1.0, 0.0), 0.0);
        let l = layout(&t);
        assert!(l.forward_diagonals && !l.backward_diagonals);
        assert_eq!(t.move_vector(&l), Some([0.0, 1.0]));
        // Slide onto the forward-left diagonal.
        t.moved(&l, 1, cell(0.0, 0.0));
        assert_eq!(t.move_vector(&layout(&t)), Some([1.0, 1.0]));
        // A world finger at the same time looks.
        t.down(&l, 2, [300.0, 100.0], 0.0);
        assert_eq!(t.aim(), Some([300.0, 100.0]));
        t.up(&l, 1, 0.5, false);
        assert_eq!(t.move_vector(&layout(&t)), None);
        assert!(!layout(&t).forward_diagonals);
    }

    #[test]
    fn back_shows_the_backward_diagonals() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        t.down(&l, 1, cell(1.0, 2.0), 0.0);
        let l = layout(&t);
        assert!(l.backward_diagonals && !l.forward_diagonals);
        t.moved(&l, 1, cell(2.0, 2.0));
        assert_eq!(t.move_vector(&layout(&t)), Some([-1.0, -1.0]));
    }

    #[test]
    fn the_diagonals_are_world_until_shown() {
        let t = TouchControls::default();
        assert_eq!(layout(&t).control_at(cell(0.0, 0.0)), None);
        assert_eq!(layout(&t).control_at(cell(2.0, 2.0)), None);
    }

    #[test]
    fn buttons() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        let centre = cell(1.0, 1.0);
        assert_eq!(
            t.down(&l, 1, centre, 0.0),
            vec![Event::Press(Control::Sneak)]
        );
        assert_eq!(t.move_vector(&l), None);
        let pause = [(W / 2.0).floor() + 5.0, 5.0];
        assert_eq!(
            t.down(&l, 2, pause, 0.0),
            vec![Event::Press(Control::Pause)]
        );
        let inventory = [(W / 2.0).floor() + 100.0, H - 10.0];
        assert!(t.down(&l, 3, inventory, 0.0).is_empty());
        assert_eq!(
            t.up(&l, 3, 0.1, false),
            vec![Event::Press(Control::Inventory)]
        );
        // Jump is held while a finger is on it.
        t.down(&l, 4, [W - 60.0, H - 90.0], 0.0);
        assert_eq!(t.held(&l), vec![Control::Jump]);
        t.set_ascend(true);
        let l = layout(&t);
        assert_eq!(t.held(&l), vec![Control::Ascend]);
    }

    #[test]
    fn hotbar_taps_select_and_holds_drop() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        let slot = |i: f32| [(W / 2.0).floor() - 91.0 + 1.0 + 20.0 * i + 10.0, H - 10.0];
        t.down(&l, 1, slot(3.0), 0.0);
        assert_eq!(t.up(&l, 1, 0.1, false), vec![Event::Slot(3)]);
        // Lifted over another slot: nothing.
        t.down(&l, 1, slot(3.0), 0.0);
        t.moved(&l, 1, slot(5.0));
        assert!(t.up(&l, 1, 0.1, false).is_empty());
        t.down(&l, 1, slot(8.0), 0.0);
        assert_eq!(t.frame(DROP_SECONDS), vec![Event::Drop]);
        assert!(t.up(&l, 1, 3.0, false).is_empty());
    }

    #[test]
    fn a_screen_releases_the_hold() {
        let mut t = TouchControls::default();
        let l = layout(&t);
        t.down(&l, 1, [200.0, 100.0], 0.0);
        t.frame(1.0);
        assert_eq!(t.clear(), vec![Event::Release]);
        assert!(t.is_empty());
        assert!(t.up(&l, 1, 2.0, false).is_empty());
    }

    #[test]
    fn the_centre_aims_where_the_view_looks() {
        for (yaw, pitch) in [(0.0f32, 0.0f32), (90.0, 30.0), (-135.0, -60.0)] {
            let v = aim_vector([W / 2.0, H / 2.0], [W, H], 70.0, yaw, pitch);
            let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
            let view = [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()];
            for i in 0..3 {
                assert!((v[i] - view[i]).abs() < 1e-9, "{v:?} {view:?}");
            }
        }
        // The top edge is half the field of view up; the left edge is to the view's left.
        let up = aim_vector([W / 2.0, 0.0], [W, H], 70.0, 0.0, 0.0);
        assert!((up[1].atan2(up[2]).to_degrees() - 35.0).abs() < 1e-6);
        let left = aim_vector([0.0, H / 2.0], [W, H], 70.0, 0.0, 0.0);
        // Yaw 0 looks along +z; its left (yaw -90's view) is +x.
        assert!(left[0] > 0.5);
    }
}
