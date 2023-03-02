pub struct ImageStore {
    pub(crate) base_path: String,
}

impl ImageStore {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }

    pub fn save(&self, image: &str, user_id: &str) {
        std::fs::write(self.file_path(user_id), image).expect("Failed to save image");
    }

    pub fn remove(&self, user_id: &str) -> std::io::Result<()> {
        std::fs::remove_file(self.file_path(user_id))
    }

    /// If any of the `user_ids` is None, the corresponding image will be None without having to read the file.
    pub fn get_many(&self, user_ids: &[Option<&str>]) -> Vec<Option<String>> {
        user_ids
            .iter()
            .map(|&user_id| match user_id {
                Some(user_id) => self.get(user_id),
                None => None,
            })
            .collect()
    }

    pub(crate) fn get(&self, user_id: &str) -> Option<String> {
        match std::fs::read_to_string(self.file_path(user_id)) {
            Ok(image) => Some(image),
            Err(_) => None,
        }
    }

    pub(crate) fn file_path(&self, user_id: &str) -> std::path::PathBuf {
        std::fs::canonicalize(&self.base_path)
            .expect("Base path is not a valid path")
            .join(user_id)
    }
}
