use anyhow::Result;

#[derive(Clone, Debug)]
pub struct ZonePresentation {
    pub slug: String,
}

impl ZonePresentation {
    pub fn load(slug: &str) -> Result<Self> {
        let root = workspace_packs_root();
        let _ = data_runtime::zone_snapshot::ZoneSnapshot::load(root, slug)?;
        Ok(Self {
            slug: slug.to_string(),
        })
    }
}

fn workspace_packs_root() -> std::path::PathBuf {
    if let Ok(override_root) = std::env::var("ROA_PACKS_ROOT_FOR_TESTS") {
        let p = std::path::PathBuf::from(override_root);
        if p.exists() {
            return p;
        }
    }
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../packs");
    if ws.exists() {
        ws
    } else {
        here.join("../..//packs")
    }
}
