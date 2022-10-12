use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;

use crate::filters::Filters;
use crate::preset::Preset;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
pub struct Dependency {
    pub url: String,
    pub refname: String,

    #[serde(flatten)]
    pub filters: Filters,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize, Clone)]
pub struct LockedDependency {
    pub url: String,
    pub refname: String,
}

impl Dependency {
    /// Creates a new dependency configuration, uses sane default values, which
    /// come pre-configured for working with proto files.
    pub fn new(url: &str, refname: &str) -> Self {
        Self {
            url: url.to_string(),
            refname: refname.to_string(),
            filters: Filters::new(),
        }
    }

    pub fn to_locked_dependency(&self, refname: &str) -> LockedDependency {
        LockedDependency::new(&self.url, refname)
    }

    /// Updates the values, taken from another dependency.
    pub fn update_from(&mut self, other: &Dependency) -> &Self {
        self.refname = other.refname.clone();
        self.filters = other.filters.clone();
        self
    }

    pub fn apply_preset(&mut self, preset: &Preset) -> &Self {
        if preset.force_filters() {
            self.filters.clear();
        }
        self.filters.merge(&preset.dependency_filters(self));
        self
    }
}

impl LockedDependency {
    pub fn new(url: &str, refname: &str) -> Self {
        Self {
            url: url.to_string(),
            refname: refname.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::svec;
    use crate::test_utils::build_preset;
    use crate::test_utils::preset_builder;

    fn get_dep_filters() -> Filters {
        let mut filters = Filters::new();
        filters
            .add_targets(&svec!("some-target"))
            .add_ignores(&svec!("some-ignore"))
            .add_extensions(&svec!("some-ext"));
        filters
    }

    #[test]
    fn test_dependency_apply_preset_without_force_filters() {
        let preset = &build_preset();
        let sut = &mut Dependency::new("some-url", "some-refname");
        sut.filters = get_dep_filters();

        let mut expected = sut.filters.clone();
        expected.merge(&preset.dependency_filters(sut));
        assert_ne!(expected, sut.filters);
        sut.apply_preset(preset);
        assert_eq!(expected, sut.filters);
    }

    #[test]
    fn test_dependency_apply_preset_with_force_filters() {
        let preset = &preset_builder().force_filters(true).build();
        let sut = &mut Dependency::new("some-url", "some-refname");
        sut.filters = get_dep_filters();

        assert_ne!(preset.dependency_filters(sut), sut.filters);
        sut.apply_preset(preset);
        assert_eq!(preset.dependency_filters(sut), sut.filters);
    }

    #[test]
    fn test_dependency_to_locked_dependency() {
        let sut = Dependency::new("some-url", "some-refname");
        let locked = sut.to_locked_dependency("other-refname");

        assert_eq!(sut.url, locked.url);
        assert_eq!("other-refname", locked.refname);
    }

    #[test]
    fn test_dependency_update_from() {
        let mut original = Dependency::new("url-a", "refname-a");
        original
            .filters
            .add_extensions(&["a".into()])
            .add_targets(&["b".into()])
            .add_ignores(&["c".into()]);

        let mut other = Dependency::new("url-b", "refname-b");
        other
            .filters
            .add_extensions(&["1".into()])
            .add_targets(&["2".into()])
            .add_ignores(&["3".into()]);

        let mut actual = original.clone();
        actual.update_from(&other);

        assert_eq!("refname-b", actual.refname);
        assert_eq!(actual.filters, other.filters);
    }
}
