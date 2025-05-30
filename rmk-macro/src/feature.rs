//! Utilities of check cargo feature
//!

/// Get enabled RMK features list
pub(crate) fn get_rmk_features() -> Option<Vec<String>> {
    match cargo_toml::Manifest::from_path("./Cargo.toml") {
        Ok(manifest) => manifest
            .dependencies
            .iter()
            .find(|(name, _dep)| *name == "rmk")
            .map(|(_name, dep)| {
                let default_features = if let Some(d) = dep.detail() {
                    d.default_features
                } else {
                    true
                };

                let mut feature_set = dep.req_features().to_vec();

                // Add default features to the feature list
                if default_features {
                    feature_set.push("defmt".to_string());
                    feature_set.push("col2row".to_string());
                    feature_set.push("storage".to_string());
                }
                feature_set
            }),
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
