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

#[test]
fn maps_bit_string_type_names() {
    // PLC-85: BYTE/WORD/DWORD/LWORD are IEC bit-string types and must not fall
    // through to `Derived`, otherwise same-type assignment wrongly fails.
    assert_eq!(TypeKind::from_name("BYTE"), TypeKind::BitString(8));
    assert_eq!(TypeKind::from_name("word"), TypeKind::BitString(16));
    assert_eq!(TypeKind::from_name("DWORD"), TypeKind::BitString(32));
    assert_eq!(TypeKind::from_name("LWORD"), TypeKind::BitString(64));
    assert_eq!(TypeKind::BitString(16).display_name(), "WORD");
}

#[test]
fn bit_string_assignment_accepts_same_type_widening_and_integers() {
    // PLC-85: a real ST compiler accepts WORD := WORD, BYTE := BYTE, widening
    // (WORD := BYTE) and integer literals into bit-strings (WORD := 15).
    let byte = TypeKind::BitString(8);
    let word = TypeKind::BitString(16);
    assert!(word.assignment_compatible(&word));
    assert!(byte.assignment_compatible(&byte));
    assert!(word.assignment_compatible(&byte)); // widening
    assert!(word.assignment_compatible(&TypeKind::Integer)); // integer literal
    // Narrowing loses bits and is rejected (matches CODESYS/MATIEC).
    assert!(!byte.assignment_compatible(&word));
    // A real value is not a bit string.
    assert!(!word.assignment_compatible(&TypeKind::Real));
}

#[test]
fn same_named_derived_types_assign_to_themselves() {
    // PLC-85: enum := enum / struct := struct of the same user type must resolve.
    let a = TypeKind::Derived("MotorState".to_owned());
    let b = TypeKind::Derived("motorstate".to_owned());
    let other = TypeKind::Derived("Color".to_owned());
    assert!(a.assignment_compatible(&b));
    assert!(!a.assignment_compatible(&other));
}
