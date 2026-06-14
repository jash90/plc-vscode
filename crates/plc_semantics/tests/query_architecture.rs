use plc_semantics::{QueryDurability, SemanticQueryDatabase, SourceSnapshot};

#[test]
fn memoizes_parse_index_and_type_check_queries() {
    let mut db = SemanticQueryDatabase::new();
    let snapshots = [SourceSnapshot::user_code(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\nEND_PROGRAM\n",
    )];

    let first = db.analyze(&snapshots);
    let after_first = db.stats();
    let second = db.analyze(&snapshots);
    let after_second = db.stats();

    assert_eq!(first, second);
    assert_eq!(after_first.parse_runs, 1);
    assert_eq!(after_first.index_and_type_check_runs, 1);
    assert_eq!(after_second, after_first);
}

#[test]
fn whitespace_only_changes_reuse_query_results() {
    let mut db = SemanticQueryDatabase::new();
    let original = [SourceSnapshot::user_code(
        "file:///main.st",
        1,
        "PROGRAM Main\nVAR\nEnabled : BOOL;\nEND_VAR\nEND_PROGRAM\n",
    )];
    let whitespace_only = [SourceSnapshot::user_code(
        "file:///main.st",
        2,
        "PROGRAM Main\nVAR\n    Enabled : BOOL;\nEND_VAR\n\nEND_PROGRAM\n",
    )];

    let _ = db.analyze(&original);
    let after_original = db.stats();
    let _ = db.analyze(&whitespace_only);
    let after_whitespace = db.stats();

    assert_eq!(after_original, after_whitespace);
}

#[test]
fn distinguishes_standard_library_from_user_code_durability() {
    let stdlib =
        SourceSnapshot::standard_library("memory://stdlib.st", "FUNCTION LIMIT\nEND_FUNCTION\n");
    let user = SourceSnapshot::user_code("file:///stdlib.st", 1, "FUNCTION LIMIT\nEND_FUNCTION\n");

    assert_eq!(stdlib.durability, QueryDurability::StandardLibrary);
    assert_eq!(user.durability, QueryDurability::UserCode);
}
