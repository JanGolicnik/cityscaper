// #![windows_subsystem = "windows"]

use application::Application;
use jandering_engine::engine::Engine;

mod application;
mod camera_controller;
mod color_obj;
mod cylinder;
mod icosphere;
mod image;
mod l_system;
mod render_data;

fn main() {
    let mut engine = pollster::block_on(Engine::new());

    let app = pollster::block_on(Application::new(&mut engine));

    pollster::block_on(engine.run(app));
}
