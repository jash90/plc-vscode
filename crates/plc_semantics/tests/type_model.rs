use plc_semantics::TypeKind;

#[test]
fn maps_elementary_type_names() {
    assert_eq!(TypeKind::from_name("BOOL"), TypeKind::Bool);
    assert_eq!(TypeKind::from_name("dint"), TypeKind::Integer);
    assert_eq!(TypeKind::from_name("LREAL"), TypeKind::Real);
    assert_eq!(TypeKind::from_name("STRING"), TypeKind::String);
    assert_eq!(TypeKind::from_name("WSTRING"), TypeKind::WString);
    assert_eq!(TypeKind::from_name("TIME"), TypeKind::Time);
}

#[test]
fn maps_derived_type_names() {
    assert_eq!(
        TypeKind::from_name("MotorState"),
        TypeKind::Derived("MotorState".to_owned())
    );
}

#[test]
fn display_names_are_stable() {
    assert_eq!(TypeKind::Bool.display_name(), "BOOL");
    assert_eq!(TypeKind::WString.display_name(), "WSTRING");
    assert_eq!(
        TypeKind::Derived("MotorState".to_owned()).display_name(),
        "MotorState"
    );
}

#[test]
fn assignment_compatibility_follows_widening_rules() {
    assert!(TypeKind::Integer.assignment_compatible(&TypeKind::Integer));
    assert!(TypeKind::Real.assignment_compatible(&TypeKind::Integer));
    assert!(TypeKind::WString.assignment_compatible(&TypeKind::String));
    assert!(!TypeKind::Bool.assignment_compatible(&TypeKind::String));
    assert!(!TypeKind::Integer.assignment_compatible(&TypeKind::Real));
}
