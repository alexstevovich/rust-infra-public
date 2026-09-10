#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Normal,
    Collapse,
    Omit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternRule {
    pub pattern: String,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Default)]
pub struct VisibilityRules {
    pub rules: Vec<PatternRule>,
}

impl VisibilityRules {
    pub fn visibility_for(&self, name: &str) -> Visibility {
        let mut visibility = Visibility::Normal;
        for rule in &self.rules {
            if matches_name(&rule.pattern, name) {
                if rule.visibility == Visibility::Omit {
                    return Visibility::Omit;
                }
                visibility = Visibility::Collapse;
            }
        }
        visibility
    }
}

fn matches_name(pattern: &str, name: &str) -> bool {
    let (pattern, name) = (pattern.as_bytes(), name.as_bytes());
    let (mut pattern_index, mut name_index) = (0, 0);
    let (mut star_index, mut star_match) = (None, 0);

    while name_index < name.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == name[name_index] {
            pattern_index += 1;
            name_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_match = name_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_match += 1;
            name_index = star_match;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(items: &[(&str, Visibility)]) -> VisibilityRules {
        VisibilityRules {
            rules: items
                .iter()
                .map(|(pattern, visibility)| PatternRule {
                    pattern: (*pattern).into(),
                    visibility: *visibility,
                })
                .collect(),
        }
    }

    #[test]
    fn exact_rules_do_not_use_substrings() {
        let rules = rules(&[
            (".git", Visibility::Collapse),
            ("obj", Visibility::Collapse),
            ("cache", Visibility::Collapse),
        ]);
        assert_eq!(rules.visibility_for(".git"), Visibility::Collapse);
        assert_eq!(rules.visibility_for(".gitignore"), Visibility::Normal);
        assert_eq!(rules.visibility_for("object_info.rst"), Visibility::Normal);
        assert_eq!(rules.visibility_for("cachefile.rst"), Visibility::Normal);
    }

    #[test]
    fn star_matches_within_one_name() {
        let rules = rules(&[("*.egg-info", Visibility::Collapse)]);
        assert_eq!(
            rules.visibility_for("blender_mcp.egg-info"),
            Visibility::Collapse
        );
        assert_eq!(rules.visibility_for("foo.egg-info"), Visibility::Collapse);
        assert_eq!(rules.visibility_for("foo.egg-info.txt"), Visibility::Normal);
    }

    #[test]
    fn omit_wins_over_exact_or_wildcard_collapse() {
        let rules = rules(&[("*", Visibility::Collapse), ("api", Visibility::Omit)]);
        assert_eq!(rules.visibility_for("api"), Visibility::Omit);
    }
}
