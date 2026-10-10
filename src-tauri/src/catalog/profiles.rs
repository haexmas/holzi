//! The model profiles of the catalog (`_meta.profiles` in `model_catalog.json`; spec 043 FR-027,
//! research R11, ADR 0002): one suggestion for desktops and two for phones, the smaller one for
//! phones with little memory. On a phone the suggestion only chooses between the two phone
//! profiles; the fit to the memory decides which.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::platform::ModelPresets;

/// The catalog ids of the three profiles.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Profiles {
    pub desktop: String,
    pub mobile: String,
    pub mobile_low_memory: String,
}

impl Profiles {
    /// The ids a suggestion may choose from under `presets`; `None` means every entry.
    pub fn candidates(&self, presets: ModelPresets) -> Option<[&str; 2]> {
        match presets {
            ModelPresets::Desktop => None,
            ModelPresets::Phone => Some([&self.mobile, &self.mobile_low_memory]),
        }
    }
}

#[derive(Deserialize)]
struct RawMeta {
    #[serde(rename = "_meta")]
    meta: Meta,
}

#[derive(Deserialize)]
struct Meta {
    profiles: Profiles,
}

static PROFILES: OnceLock<Profiles> = OnceLock::new();

/// The profiles of the built-in catalog.
pub fn profiles() -> &'static Profiles {
    PROFILES.get_or_init(|| {
        let raw: RawMeta = serde_json::from_str(super::CATALOG_JSON)
            .expect("built-in catalog JSON must name its profiles");
        raw.meta.profiles
    })
}

#[cfg(test)]
#[path = "profiles_tests.rs"]
mod profiles_tests;
