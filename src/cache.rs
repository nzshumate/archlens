//! Content-validated parser cache. Resolution and graph construction always run fresh.
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::Write, path::Path};

const VERSION: u32 = 1;
#[derive(Clone, Deserialize, Serialize)]
pub struct ParsedSource {
    pub source: String,
    pub imports: Vec<String>,
    pub lines: usize,
}
#[derive(Deserialize, Serialize)]
pub struct Cache {
    version: u32,
    pub entries: HashMap<String, ParsedSource>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            version: VERSION,
            entries: HashMap::new(),
        }
    }
}
impl Cache {
    pub fn load(root: &Path) -> Self {
        fs::read(root.join("target/oxarch/parsed-v1.json"))
            .ok()
            .and_then(|raw| serde_json::from_slice::<Self>(&raw).ok())
            .filter(|cache| cache.version == VERSION)
            .unwrap_or_default()
    }
    pub fn save(&self, root: &Path) {
        let dir = root.join("target/oxarch");
        // Cache failures must never prevent analysis. A temporary file and rename
        // keep concurrent readers from observing partially serialized data.
        let write = || -> anyhow::Result<()> {
            fs::create_dir_all(&dir)?;
            let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
            temp.write_all(&serde_json::to_vec(self)?)?;
            temp.persist(dir.join("parsed-v1.json"))?;
            Ok(())
        };
        let _ = write();
    }
}
