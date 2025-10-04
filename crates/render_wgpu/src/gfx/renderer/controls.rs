//! Helpers for parsing controller profile from config.

pub fn parse_profile_name(name: Option<&str>) -> ecs_core::components::InputProfile {
    use ecs_core::components::InputProfile::{ActionCombat, ClassicCursor};
    match name.map(|s| s.trim().to_ascii_lowercase()) {
        Some(s) if s == "classic" || s == "classiccursor" => ClassicCursor,
        Some(s) if s == "action" || s == "actioncombat" => ActionCombat,
        _ => ActionCombat,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_profile_name;
    use ecs_core::components::InputProfile;
    #[test]
    fn parse_profile_variants() {
        assert_eq!(
            parse_profile_name(Some("ActionCombat")),
            InputProfile::ActionCombat
        );
        assert_eq!(
            parse_profile_name(Some("classic")),
            InputProfile::ClassicCursor
        );
        assert_eq!(
            parse_profile_name(Some("ClassicCursor")),
            InputProfile::ClassicCursor
        );
        assert_eq!(parse_profile_name(None), InputProfile::ActionCombat);
        assert_eq!(
            parse_profile_name(Some("unknown")),
            InputProfile::ActionCombat
        );
    }
}
