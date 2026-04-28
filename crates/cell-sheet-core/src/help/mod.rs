pub mod entries;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpCategory {
    Normal,
    Insert,
    Visual,
    Command,
    Formula,
}

impl HelpCategory {
    pub fn label(&self) -> &'static str {
        match self {
            HelpCategory::Normal => "NORMAL MODE",
            HelpCategory::Insert => "INSERT MODE",
            HelpCategory::Visual => "VISUAL MODE",
            HelpCategory::Command => "COMMANDS",
            HelpCategory::Formula => "FORMULAS",
        }
    }
}

#[derive(Debug)]
pub struct HelpEntry {
    pub tags: &'static [&'static str],
    pub category: HelpCategory,
    pub summary: &'static str,
    pub detail: &'static str,
}

pub struct HelpRegistry {
    entries: Vec<&'static HelpEntry>,
}

impl Default for HelpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpRegistry {
    /// Build the default registry with all built-in help entries.
    pub fn new() -> Self {
        use entries::*;
        Self::from_entries(&[
            NORMAL_ENTRIES,
            INSERT_ENTRIES,
            VISUAL_ENTRIES,
            COMMAND_ENTRIES,
            FORMULA_ENTRIES,
        ])
    }

    /// Build a registry from multiple static entry slices (one per module).
    pub fn from_entries(slices: &[&'static [HelpEntry]]) -> Self {
        let mut entries = Vec::new();
        for slice in slices {
            for entry in *slice {
                entries.push(entry);
            }
        }
        HelpRegistry { entries }
    }

    /// Find an entry by tag (case-insensitive).
    pub fn find(&self, tag: &str) -> Option<&'static HelpEntry> {
        let tag_lower = tag.to_lowercase();
        for entry in &self.entries {
            for t in entry.tags {
                if t.to_lowercase() == tag_lower {
                    return Some(entry);
                }
            }
        }
        None
    }

    /// Return all entries in a given category, in registration order.
    pub fn by_category(&self, category: HelpCategory) -> Vec<&'static HelpEntry> {
        self.entries
            .iter()
            .copied()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Return all categories that have at least one entry, in display order.
    pub fn categories(&self) -> Vec<HelpCategory> {
        use HelpCategory::*;
        let order = [Normal, Insert, Visual, Command, Formula];
        order
            .iter()
            .copied()
            .filter(|cat| self.entries.iter().any(|e| e.category == *cat))
            .collect()
    }

    /// Return all entries in display order (grouped by category).
    pub fn all_entries(&self) -> &[&'static HelpEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_ENTRIES: &[HelpEntry] = &[
        HelpEntry {
            tags: &["h"],
            category: HelpCategory::Normal,
            summary: "Move cursor left",
            detail: "Move the cursor one column to the left.",
        },
        HelpEntry {
            tags: &[":w", ":write"],
            category: HelpCategory::Command,
            summary: "Save file",
            detail: "Write the current sheet to disk.",
        },
    ];

    #[test]
    fn find_by_tag() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        let entry = registry.find("h").unwrap();
        assert_eq!(entry.summary, "Move cursor left");
    }

    #[test]
    fn find_by_alias_tag() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        let entry = registry.find(":write").unwrap();
        assert_eq!(entry.summary, "Save file");
    }

    #[test]
    fn find_case_insensitive() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        let entry = registry.find("H").unwrap();
        assert_eq!(entry.summary, "Move cursor left");
    }

    #[test]
    fn find_not_found() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        assert!(registry.find("zzz").is_none());
    }

    #[test]
    fn by_category() {
        let registry = HelpRegistry::from_entries(&[TEST_ENTRIES]);
        let normals = registry.by_category(HelpCategory::Normal);
        assert_eq!(normals.len(), 1);
        assert_eq!(normals[0].tags[0], "h");
    }

    #[test]
    fn full_registry_has_expected_tags() {
        let registry = HelpRegistry::new();
        assert!(registry.find("h").is_some(), "missing h");
        assert!(registry.find("dd").is_some(), "missing dd");
        assert!(registry.find(":w").is_some(), "missing :w");
        assert!(registry.find(":help").is_some(), "missing :help");
        assert!(registry.find("SUM").is_some(), "missing SUM");
        assert!(registry.find("IF").is_some(), "missing IF");
        assert!(registry.find("Esc").is_some(), "missing Esc");
        assert!(registry.find("v").is_some(), "missing v");
        assert!(
            registry.find(":set delimiter").is_some(),
            "missing :set delimiter"
        );
    }
}
