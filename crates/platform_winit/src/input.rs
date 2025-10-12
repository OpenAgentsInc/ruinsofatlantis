#![allow(dead_code)]
/// Example pure mapping used for a unit test; real mapping remains in legacy code.
pub fn map_keys_to_intent(keys: &[&str]) -> Option<&'static str> {
    if keys.contains(&"W") {
        Some("MoveForward")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::map_keys_to_intent;
    #[test]
    fn maps_w_to_move_forward() {
        assert_eq!(map_keys_to_intent(&["W"]), Some("MoveForward"));
        assert_eq!(map_keys_to_intent(&["A"]), None);
    }
}
