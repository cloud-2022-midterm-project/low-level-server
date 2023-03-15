#![allow(non_snake_case)]
use std::path::Path;

pub mod app_state;
mod handlers;
pub mod image;
mod models;
pub mod mutation_manager;
mod request;
mod response;

pub use handlers::handle_connection;

pub fn try_write_perm(path: &Path) {
    let test_file_path = path.join("test_file.txt");
    std::fs::write(&test_file_path, "test").expect(
        format!(
            "Failed to write to {}. Try `sudo chmod 777 {}",
            path.display(),
            path.display()
        )
        .as_str(),
    );
    std::fs::remove_file(&test_file_path).unwrap();
}
