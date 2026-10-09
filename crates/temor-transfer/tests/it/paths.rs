use temor_transfer::{FileName, RelPath};

#[test]
fn relative_paths_parse_component_by_component() {
    let path = RelPath::parse("a/b c/d.txt").expect("valid");
    assert_eq!(
        path.components()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["a", "b c", "d.txt"]
    );
    assert_eq!(path.leaf(), "d.txt");
    let joined = RelPath::parse("x").expect("valid").join(&path);
    assert_eq!(
        joined
            .components()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["x", "a", "b c", "d.txt"]
    );
    assert_eq!(RelPath::root().join(&path), path);
}

#[test]
fn absolute_empty_dot_and_dotdot_paths_are_refused() {
    for raw in [
        "",
        "/",
        "/etc/passwd",
        "a/",
        "a//b",
        "./a",
        "a/./b",
        "..",
        "../x",
        "a/..",
        "a/../../b",
        "a\\b",
        "..\\x",
        "a\nb",
        "a\0b",
        "\u{7}",
    ] {
        assert!(RelPath::parse(raw).is_err(), "{raw:?} must be refused");
    }
}

#[test]
fn file_names_are_always_safe_to_create() {
    for raw in [
        "../../etc/passwd",
        "..",
        ".",
        "",
        "   ",
        "/abs/name",
        "C:\\win\\name",
        "tab\tand\nnewline",
        "\u{0}nul",
    ] {
        let name = FileName::sanitise(raw);
        let text = name.as_str();
        assert!(!text.is_empty(), "{raw:?} gave an empty name");
        assert!(
            !text.contains('/') && !text.contains('\\'),
            "{raw:?} kept a separator"
        );
        assert!(!text.starts_with('.'), "{raw:?} kept a leading dot");
        assert!(
            !text.chars().any(char::is_control),
            "{raw:?} kept a control character"
        );
    }
}

#[test]
fn long_names_are_cut_on_a_character_boundary() {
    let name = FileName::sanitise(&"é".repeat(300));
    assert!(name.as_str().len() <= 200);
    assert!(name.as_str().chars().all(|ch| ch == 'é'));
}

#[test]
fn an_ordinary_name_is_kept_as_it_is() {
    assert_eq!(
        FileName::sanitise("report 2026.pdf").as_str(),
        "report 2026.pdf"
    );
}

#[test]
fn the_protocol_length_limits_hold() {
    assert!(RelPath::parse(&"a".repeat(255)).is_ok());
    assert!(RelPath::parse(&"a".repeat(256)).is_err());
    let deep = vec!["ab"; 2100].join("/");
    assert!(deep.len() > 4096);
    assert!(RelPath::parse(&deep).is_err());
}

#[test]
fn components_starting_with_a_dot_are_refused() {
    for raw in [".ssh/x", "a/.git/config", ".bashrc", "a/..b"] {
        assert!(RelPath::parse(raw).is_err(), "{raw:?} must be refused");
    }
}
