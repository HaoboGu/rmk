//! Utilities of check cargo feature
//!

/// Get enabled RMK features list
pub(crate) fn get_rmk_features() -> Option<Vec<String>> {
    match cargo_toml::Manifest::from_path("./Cargo.toml") {
        Ok(manifest) => manifest
            .dependencies
            .iter()
            .find(|(name, _dep)| *name == "rmk")
            .map(|(_name, dep)| dep.req_features().to_vec()),
        Err(_e) => None,
    }
}

/// Check whether the given feature is enabled
pub(crate) fn is_feature_enabled(feature_list: &Option<Vec<String>>, feature: &str) -> bool {
    if let Some(rmk_features) = feature_list {
        for f in rmk_features {
            if f == feature {
                return true;
            }
        }
    }
    false
}
