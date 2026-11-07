use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Storage {
    pub dir: PathBuf,
    pub public_base_url: String,
}

impl Storage {
    pub fn new(dir: PathBuf, public_base_url: String) -> Self {
        let _ = fs::create_dir_all(&dir);
        Self {
            dir,
            public_base_url,
        }
    }

    pub fn path_for(&self, id: Uuid) -> PathBuf {
        self.dir.join(id.to_string())
    }

    pub fn url_for(&self, id: Uuid) -> String {
        format!(
            "{}/uploads/{}",
            self.public_base_url.trim_end_matches('/'),
            id
        )
    }

    pub fn ensure_dir(&self) -> Result<(), String> {
        fs::create_dir_all(&self.dir)
            .map_err(|error| format!("Не удалось создать папку файлов: {error}"))
    }

    pub fn path_exists(&self, id: Uuid) -> bool {
        self.path_for(id).exists()
    }

    pub fn root(&self) -> &Path {
        &self.dir
    }
}
