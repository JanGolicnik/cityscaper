use jandering_engine::{
    types::{Vec2, Vec3},
    {
        bind_group::camera::free::CameraController,
        window::{InputState, Key, MouseButton, WindowEvent},
    },
};
const CAMERA_SPEED: f32 = 20.0;

pub struct IsometricCameraController {
    pub pan_speed: f32,

    pub right_pressed: bool,
    pub left_pressed: bool,
    pub forward_pressed: bool,
    pub backward_pressed: bool,
    pub is_shift_pressed: bool,
    pub speed_multiplier: f32,
    pub velocity: Vec3,

    pub last_mouse_position: Option<Vec2>,
    pub mouse_down: bool,
    pub pan_delta: Vec2,
}

impl Default for IsometricCameraController {
    fn default() -> Self {
        Self {
            pan_speed: 0.03,
            right_pressed: false,
            left_pressed: false,
            forward_pressed: false,
            backward_pressed: false,
            is_shift_pressed: false,
            speed_multiplier: 1.0,
            velocity: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            last_mouse_position: None,
            mouse_down: false,
            pan_delta: Vec2::ZERO,
        }
    }
}

impl IsometricCameraController {
    fn mouse_motion(&mut self, position: Vec2) {
        if !self.mouse_down {
            return;
        }

        if let Some(last_mouse_position) = &mut self.last_mouse_position {
            self.pan_delta += position - *last_mouse_position;
        }

        self.last_mouse_position = Some(position);
    }
}

impl CameraController for IsometricCameraController {
    fn event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::MouseMotion(position) => {
                self.mouse_motion(Vec2::from(position));
            }
            WindowEvent::MouseLeft => {
                self.last_mouse_position = None;
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
            } => {
                self.mouse_down = {
                    let val = matches!(state, InputState::Pressed);
                    if !val {
                        self.last_mouse_position = None;
                    }
                    val
                };
            }
            WindowEvent::Scroll((_, val)) => {
                if val.is_sign_positive() {
                    self.speed_multiplier += (10.0 - self.speed_multiplier) / 100.0;
                } else {
                    self.speed_multiplier += (10.0 - self.speed_multiplier) / 20.0;
                }
            }

            WindowEvent::KeyInput { key, state } => {
                let is_pressed = matches!(state, InputState::Pressed);
                match key {
                    Key::A => self.left_pressed = is_pressed,
                    Key::D => self.right_pressed = is_pressed,
                    Key::S => self.forward_pressed = is_pressed,
                    Key::W => self.backward_pressed = is_pressed,
                    Key::Shift => self.is_shift_pressed = is_pressed,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn update(&mut self, object_position: &mut Vec3, object_direction: &mut Vec3, dt: f32) {
        let Self {
            right_pressed,
            left_pressed,
            forward_pressed,
            backward_pressed,
            is_shift_pressed,
            speed_multiplier,
            ..
        } = *self;

        let dir = *object_direction;
        let right = dir.cross(Vec3::Y).normalize();

        let speed = CAMERA_SPEED * speed_multiplier * if is_shift_pressed { 2.0 } else { 1.0 };
        self.velocity = Vec3::new(
            if left_pressed {
                -speed
            } else if right_pressed {
                speed
            } else {
                self.velocity.x
            },
            if forward_pressed {
                -speed
            } else if backward_pressed {
                speed
            } else {
                self.velocity.y
            },
            0.0,
        );

        let pan_delta = self.pan_delta * self.pan_speed;
        self.pan_delta = Vec2::ZERO;

        let dir = Vec3::new(dir.x, 0.0, dir.z).normalize();
        *object_position += -pan_delta.x * right + self.velocity.x * right * dt;
        *object_position += pan_delta.y * dir + self.velocity.y * dir * dt;

        self.velocity += -self.velocity * (dt * 6.0);
    }
}
