use application::ApplicationBuilder;
use jandering_engine::{engine::EngineBuilder, window::WindowConfig};

mod application;
mod camera_controller;
mod color_obj;
mod cylinder;
mod icosphere;
mod image;
mod l_system;
mod render_data;

fn main() {
    let app_builder = pollster::block_on(ApplicationBuilder::new());

    EngineBuilder::default()
        .with_window(
            WindowConfig::default()
                .with_cursor(true)
                .with_auto_resolution()
                .with_title("heyy")
                .with_cursor(true),
        )
        .run(app_builder);
}
