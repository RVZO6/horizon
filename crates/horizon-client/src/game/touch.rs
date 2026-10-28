//! Touch input: in a menu the first finger is the mouse; in play the touch controls
//! (`horizon_client::touch`) drive the player through the gamepad's actions.
use super::{Game, view_vector};
use horizon_client::{
    hud::{Blend, HudBatch},
    touch::{self, Control, Event, Layout, RING_RADIUS, TouchControls},
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceId, ElementState, MouseButton, Touch, TouchPhase, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::WindowId,
};

/// A finger after the vanilla mouse: the events a menu reads, for the first finger down.
pub(super) fn touch_as_mouse(finger: &mut Option<u64>, touch: &Touch) -> Vec<WindowEvent> {
    if finger.is_some_and(|f| f != touch.id) {
        return Vec::new();
    }
    let moved = WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: touch.location,
    };
    let button = |state| WindowEvent::MouseInput {
        device_id: DeviceId::dummy(),
        state,
        button: MouseButton::Left,
    };
    match touch.phase {
        TouchPhase::Started if finger.is_none() => {
            *finger = Some(touch.id);
            vec![moved, button(ElementState::Pressed)]
        }
        TouchPhase::Started => Vec::new(),
        TouchPhase::Moved => vec![moved],
        TouchPhase::Ended | TouchPhase::Cancelled => {
            *finger = None;
            vec![moved, button(ElementState::Released)]
        }
    }
}

/// A tap's target stays aimed at this long after the finger lifts: the attack it starts
/// happens on the next tick.
const TAP_AIM_SECONDS: f64 = 0.15;

#[derive(Default)]
pub(super) struct TouchState {
    controls: TouchControls,
    /// Touch was the last input: the controls show and the crosshair hides.
    pub(super) active: bool,
    /// The finger that is the mouse in a menu.
    menu_finger: Option<u64>,
    /// What the hold pressed, until the finger lifts.
    holding: Option<MouseButton>,
    jump: bool,
    descend: bool,
    /// Where the last tap aimed, and when.
    tapped: Option<([f32; 2], f64)>,
}

impl Game<'_> {
    pub(super) fn touch_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, t: Touch) {
        self.touch.active = true;
        if self.touch.menu_finger.is_some()
            || (self.pad_in_menu() && t.phase == TouchPhase::Started)
        {
            for event in touch_as_mouse(&mut self.touch.menu_finger, &t) {
                self.window_event(event_loop, id, event);
            }
            return;
        }
        let Some(layout) = self.touch_layout() else {
            return;
        };
        let scale = HudBatch::new(self.window_size()).scale;
        let at = [t.location.x as f32 / scale, t.location.y as f32 / scale];
        let now = self.started.elapsed().as_secs_f64();
        let events = match t.phase {
            TouchPhase::Started => self.touch.controls.down(&layout, t.id, at, now),
            TouchPhase::Moved => {
                self.touch.controls.moved(&layout, t.id, at);
                Vec::new()
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let aim = self.touch.controls.aim();
                let events =
                    self.touch
                        .controls
                        .up(&layout, t.id, now, t.phase == TouchPhase::Cancelled);
                if let Some(at) = aim.filter(|_| events.contains(&Event::Tap)) {
                    self.touch.tapped = Some((at, now));
                }
                events
            }
        };
        self.touch_events(events);
    }

    /// Once a frame: holds, the view's turn and the held buttons.
    pub(super) fn touch_frame(&mut self) {
        if self.pad_in_menu() {
            let events = self.touch.controls.clear();
            self.touch_events(events);
            self.touch_keys(false, false);
            return;
        }
        let ascend = self
            .player
            .as_ref()
            .is_some_and(|p| p.flying || p.fluids.was_touching_water);
        self.touch.controls.set_ascend(ascend);
        let events = self
            .touch
            .controls
            .frame(self.started.elapsed().as_secs_f64());
        self.touch_events(events);
        let [yaw, pitch] = self.touch.controls.take_look();
        if yaw != 0.0 || pitch != 0.0 {
            let mut rotation = horizon_client::mouse::Rotation {
                x_rot: self.pitch,
                y_rot: self.yaw,
                x_rot_o: self.pitch,
                y_rot_o: self.yaw,
            };
            rotation.turn(f64::from(yaw / 0.15), f64::from(pitch / 0.15));
            self.yaw = rotation.y_rot;
            self.pitch = rotation.x_rot;
        }
        let Some(layout) = self.touch_layout() else {
            return;
        };
        let held = self.touch.controls.held(&layout);
        let jump = held.contains(&Control::Jump) || held.contains(&Control::Ascend);
        self.touch_keys(jump, held.contains(&Control::Descend));
    }

    /// The D-pad as the left stick, while playing.
    pub(super) fn touch_move(&self) -> Option<[f32; 2]> {
        if self.pad_in_menu() {
            return None;
        }
        self.touch.controls.move_vector(&self.touch_layout()?)
    }

    /// The direction picks follow (`Entity.getViewVector`): through the finger on the world
    /// while there is one, else the view's.
    pub(super) fn aim_vector(&self) -> [f64; 3] {
        let view = view_vector(self.yaw, self.pitch);
        let now = self.started.elapsed().as_secs_f64();
        let tapped = self
            .touch
            .tapped
            .filter(|(_, at)| now - at < TAP_AIM_SECONDS)
            .map(|(p, _)| p);
        let Some(at) = self.touch.controls.aim().or(tapped) else {
            return view;
        };
        let Some((_, _, _, fov)) = self.locator_camera else {
            return view;
        };
        let [w, h] = self.window_size();
        let scale = HudBatch::new([w, h]).scale;
        let size = [w as f32 / scale, h as f32 / scale];
        touch::aim_vector(at, size, fov, self.yaw, self.pitch)
    }

    fn window_size(&self) -> [u32; 2] {
        self.renderer
            .as_ref()
            .map_or([1, 1], |r| [r.config.width, r.config.height])
    }

    fn touch_layout(&self) -> Option<Layout> {
        let batch = self.gui_scale()?;
        Some(self.touch.controls.layout(batch.width, batch.height))
    }

    fn touch_keys(&mut self, jump: bool, descend: bool) {
        if jump != self.touch.jump {
            self.touch.jump = jump;
            self.touch_action("key.jump", jump);
        }
        if descend != self.touch.descend {
            self.touch.descend = descend;
            self.touch_key(KeyCode::ShiftLeft, descend);
        }
    }

    fn touch_events(&mut self, events: Vec<Event>) {
        for event in events {
            match event {
                Event::Press(Control::Sneak) => {
                    let sneaking = self.keys.contains(&KeyCode::ShiftLeft);
                    if self.settings.toggle_crouch {
                        self.touch_key(KeyCode::ShiftLeft, true);
                        self.touch_key(KeyCode::ShiftLeft, false);
                    } else {
                        self.touch_key(KeyCode::ShiftLeft, !sneaking);
                    }
                }
                Event::Press(Control::Chat) => self.touch_click("key.chat"),
                Event::Press(Control::Inventory) => self.touch_click("key.inventory"),
                Event::Press(Control::Pause) => {
                    if let Err(e) = self.pad_back() {
                        log::warn!("Horizon: touch pause: {e}");
                    }
                }
                Event::Press(_) => {}
                Event::Slot(slot) => self.selected = slot,
                Event::Drop => self.touch_click("key.drop"),
                // Bedrock's tap: attack the entity, else use where the finger landed.
                Event::Tap => {
                    let button = if self.pick_entity_hit().is_some() {
                        MouseButton::Left
                    } else {
                        MouseButton::Right
                    };
                    self.touch_mouse(button, true);
                    self.touch_mouse(button, false);
                }
                // TouchController's hold: keep using an item that is held to use (food, a
                // bow, a shield), else mine the block; on an entity, use it once.
                Event::Hold => {
                    if self.pick_entity_hit().is_some() {
                        self.touch_mouse(MouseButton::Right, true);
                        self.touch_mouse(MouseButton::Right, false);
                        self.touch.controls.hold_unused();
                    } else if self.held_item_is_used_over_time() {
                        self.touch_mouse(MouseButton::Right, true);
                        self.touch.holding = Some(MouseButton::Right);
                    } else if self.pick().is_some() {
                        self.touch_mouse(MouseButton::Left, true);
                        self.touch.holding = Some(MouseButton::Left);
                    } else {
                        self.touch.controls.hold_unused();
                    }
                }
                Event::Release => {
                    if let Some(button) = self.touch.holding.take() {
                        self.touch_mouse(button, false);
                    }
                }
            }
        }
    }

    fn touch_mouse(&mut self, button: MouseButton, down: bool) {
        if let Err(e) = self.play_mouse(button, down) {
            log::warn!("Horizon: touch {button:?}: {e}");
        }
    }

    fn touch_key(&mut self, key: KeyCode, down: bool) {
        if let Err(e) = self.mapped_key(key, down, false) {
            log::warn!("Horizon: touch {key:?}: {e}");
        }
    }

    fn touch_action(&mut self, action: &str, down: bool) {
        if let Err(e) = self.pad_action(action, down) {
            log::warn!("Horizon: touch {action}: {e}");
        }
    }

    fn touch_click(&mut self, action: &str) {
        self.touch_action(action, true);
        self.touch_action(action, false);
    }

    /// The controls over the HUD, and the touch ring where the finger on the world is.
    pub(super) fn draw_touch_controls(&self, batch: &mut HudBatch) {
        if !self.touch.active {
            return;
        }
        let atlas = &self.gui.atlas;
        let layout = self.touch.controls.layout(batch.width, batch.height);
        let held = self.touch.controls.held(&layout);
        let sneaking = self.keys.contains(&KeyCode::ShiftLeft);
        let swimming = self
            .player
            .as_ref()
            .is_some_and(|p| !p.flying && p.fluids.was_touching_water);
        for (control, [x, y, w, h]) in layout.controls() {
            let pressed = held.contains(&control) || (control == Control::Sneak && sneaking);
            // TouchController's `new` textures (tools/touch_textures.py) at 60% opacity, their
            // active ones while pressed.
            let sprite = format!(
                "gui/sprites/touch/{}{}",
                texture(control, swimming),
                if pressed { "_active" } else { "" }
            );
            if atlas.sprites.contains_key(&sprite) {
                batch.sprite_scaled(atlas, &sprite, x, y, w, h, 0x99_FFFFFF);
                continue;
            }
            // Without them, plain buttons greyed while pressed.
            let (edge, face, icon) = if pressed {
                (0x99_2A2A2A, 0x99_6B6B6B, 0xCC_1E1E1E)
            } else {
                (0x99_3F3F3F, 0x99_A0A0A0, 0xCC_2E2E2E)
            };
            batch.fill(atlas, x, y, w, h, edge);
            batch.fill(atlas, x + 1.0, y + 1.0, w - 2.0, h - 2.0, face);
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            let r = w * 0.25;
            let tri = |batch: &mut HudBatch, p: [[f32; 2]; 3]| {
                batch.fill_quad(atlas, [p[0], p[1], p[2], p[2]], icon);
            };
            let diamond = |batch: &mut HudBatch, r: f32, color: u32| {
                let corners = [[cx, cy - r], [cx + r, cy], [cx, cy + r], [cx - r, cy]];
                batch.fill_quad(atlas, corners, color);
            };
            match control {
                Control::Forward | Control::Ascend => {
                    tri(batch, [[cx, cy - r], [cx + r, cy + r], [cx - r, cy + r]]);
                }
                Control::Back | Control::Descend => {
                    tri(batch, [[cx - r, cy - r], [cx + r, cy - r], [cx, cy + r]]);
                }
                Control::Left => tri(batch, [[cx - r, cy], [cx + r, cy - r], [cx + r, cy + r]]),
                Control::Right => tri(batch, [[cx + r, cy], [cx - r, cy + r], [cx - r, cy - r]]),
                Control::ForwardLeft => {
                    tri(
                        batch,
                        [[cx - r, cy - r], [cx + r, cy - r], [cx - r, cy + r]],
                    );
                }
                Control::BackLeft => {
                    tri(
                        batch,
                        [[cx - r, cy + r], [cx - r, cy - r], [cx + r, cy + r]],
                    );
                }
                Control::BackRight => {
                    tri(
                        batch,
                        [[cx + r, cy + r], [cx - r, cy + r], [cx + r, cy - r]],
                    );
                }
                Control::ForwardRight => {
                    tri(
                        batch,
                        [[cx + r, cy - r], [cx + r, cy + r], [cx - r, cy - r]],
                    );
                }
                Control::Sneak => {
                    diamond(batch, r, icon);
                    if !sneaking {
                        diamond(batch, r - 3.0, face);
                    }
                }
                Control::Jump => diamond(batch, r, icon),
                Control::Chat => {
                    for i in 0..3 {
                        let ly = y + 5.0 + 3.0 * i as f32;
                        batch.fill(atlas, x + 4.0, ly, w - 8.0, 1.0, icon);
                    }
                }
                Control::Pause => {
                    batch.fill(atlas, x + 5.0, y + 4.0, 3.0, h - 8.0, icon);
                    batch.fill(atlas, x + w - 8.0, y + 4.0, 3.0, h - 8.0, icon);
                }
                Control::Inventory => {
                    for i in -1..=1 {
                        let dx = cx - 1.5 + 5.0 * i as f32;
                        batch.fill(atlas, dx, cy - 1.5, 3.0, 3.0, icon);
                    }
                }
            }
        }
        if let Some([x, y]) = self.touch.controls.aim() {
            let start = batch.vertices.len();
            let point = |r: f32, i: usize| {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                [x + r * a.cos(), y + r * a.sin()]
            };
            for i in 0..24 {
                let corners = [
                    point(RING_RADIUS, i),
                    point(RING_RADIUS + 2.0, i),
                    point(RING_RADIUS + 2.0, i + 1),
                    point(RING_RADIUS, i + 1),
                ];
                batch.fill_quad(atlas, corners, u32::MAX);
            }
            let progress = self.mining.as_ref().map_or(0.0, |m| m.progress);
            if self.touch.holding == Some(MouseButton::Left) && progress > 0.0 {
                let r = RING_RADIUS * (0.5 + 0.5 * progress.min(1.0));
                for i in 0..24 {
                    let corners = [[x, y], point(r, i), point(r, i + 1), point(r, i + 1)];
                    batch.fill_quad(atlas, corners, u32::MAX);
                }
            }
            batch.blend_since(start, Blend::Invert);
        }
    }
}

/// The control's texture in TouchController's `new` set.
fn texture(control: Control, swimming: bool) -> &'static str {
    match control {
        Control::Forward => "dpad/up",
        Control::Back => "dpad/down",
        Control::Left => "dpad/left",
        Control::Right => "dpad/right",
        Control::ForwardLeft => "dpad/up_left",
        Control::ForwardRight => "dpad/up_right",
        Control::BackLeft => "dpad/down_left",
        Control::BackRight => "dpad/down_right",
        Control::Sneak => "sneak/sneak",
        Control::Jump => "jump/jump",
        Control::Ascend if swimming => "ascend/ascend_swimming",
        Control::Ascend => "ascend/ascend",
        Control::Descend if swimming => "descend/descend_swimming",
        Control::Descend => "descend/descend",
        Control::Chat => "chat/chat",
        Control::Pause => "pause/pause",
        Control::Inventory => "inventory/inventory",
    }
}
