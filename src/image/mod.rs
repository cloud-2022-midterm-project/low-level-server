use std::{io, path::PathBuf};

pub fn file_path(base_path: &PathBuf, user_id: &str) -> PathBuf {
    std::fs::canonicalize(base_path)
        .expect("Base path is not a valid path")
        .join(user_id)
}

pub fn save(base_path: &PathBuf, image: &str, user_id: &str) -> io::Result<()> {
    std::fs::write(file_path(base_path, user_id), image)
}

pub fn remove(base_path: &PathBuf, user_id: &str) -> std::io::Result<()> {
    std::fs::remove_file(file_path(base_path, user_id))
}

pub fn get(base_path: &PathBuf, user_id: &str) -> Option<String> {
    match std::fs::read_to_string(file_path(base_path, user_id)) {
        Ok(image) => Some(image),
        Err(_) => None,
    }
}

pub fn clear(base_path: &PathBuf) -> std::io::Result<()> {
    std::fs::remove_dir_all(base_path)?;
    std::fs::create_dir(base_path)
}
