use crate::{FilterValue, entities::HttpFile};

entity!(Release(full_name: F => String, tag: T => String) {
    with_body => body: Option<String>,
    with_draft => draft: bool,
    with_prerelease => prerelease: bool,
    with_assets => assets: Vec<HttpFile>,
});

#[allow(dead_code)]
impl Release {
    /// Adds a single downloadable artifact to the release.
    pub fn with_asset(mut self, asset: HttpFile) -> Self {
        self.assets.push(asset);
        self
    }
}
