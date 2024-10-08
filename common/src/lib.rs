pub mod app;
pub mod gpu;
pub mod settings;
pub mod sim;
pub mod ui;

pub use app::App;
pub use sim::save;

pub use egui;
pub use glam;
pub use log;
pub use wgpu;

use crate::save::Project;
use crate::settings::Settings;

#[derive(
    Default, Hash, Debug, Eq, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize,
)]
pub struct Id(pub u64);
impl Id {
    pub fn new<T: std::hash::Hash>(v: T) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&v, &mut hasher);
        Self(std::hash::Hasher::finish(&hasher))
    }
}

pub trait Platform {
    fn set_scale_factor(scale: f32);

    fn load_settings() -> std::io::Result<Settings>;
    fn save_settings(settings: Settings) -> std::io::Result<()>;

    fn list_available_projects() -> std::io::Result<Vec<String>>;
    fn load_project(name: &str) -> std::io::Result<Project>;
    fn save_project(name: &str, project: Project) -> std::io::Result<()>;
    fn delete_project(name: &str) -> std::io::Result<()>;
    fn rename_project(name: &str, new_name: &str) -> std::io::Result<()>;

    fn can_open_dirs() -> bool;
    fn open_save_dir() -> std::io::Result<()>;

    fn has_external_data() -> bool;
    fn download_external_data();
    fn upload_external_data();

    fn is_touchscreen() -> bool;
    fn has_physical_keyboard() -> bool;
    fn name() -> String;
}
