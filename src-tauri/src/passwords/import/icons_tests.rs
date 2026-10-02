use super::*;

#[test]
fn every_index_from_0_to_68_has_a_name() {
    for index in 0..=68 {
        let name = standard_icon(index).unwrap_or_else(|| panic!("no name for {index}"));
        assert!(name.starts_with("lucide:"), "{index}: {name}");
    }
}

#[test]
fn an_unknown_index_has_no_name() {
    assert_eq!(standard_icon(69), None);
    assert_eq!(standard_icon(1000), None);
}

#[test]
fn every_target_name_is_a_literal_of_the_interface_list() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/passwords/icons.ts");
    let source = std::fs::read_to_string(path).expect("read icons.ts");
    for name in target_names() {
        assert!(
            source.contains(&format!("'{name}'")),
            "{name} is not a literal in src/lib/passwords/icons.ts"
        );
    }
}
