use std::collections::HashMap;

use plc_syntax::{SyntaxParse, parse_source};

use crate::{SemanticAnalysis, SourceFile, analyze_workspace};

/// Query durability class used by the incremental semantic architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryDurability {
    /// Stable built-in data such as standard library declarations.
    StandardLibrary,
    /// User-authored project files that change frequently in the editor.
    UserCode,
}

/// Versioned source snapshot stored in the semantic query database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshot {
    pub uri: String,
    pub version: i32,
    pub text: String,
    pub durability: QueryDurability,
}

impl SourceSnapshot {
    pub fn user_code(uri: impl Into<String>, version: i32, text: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            version,
            text: text.into(),
            durability: QueryDurability::UserCode,
        }
    }

    pub fn standard_library(uri: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            version: 0,
            text: text.into(),
            durability: QueryDurability::StandardLibrary,
        }
    }
}

/// Query execution counters used by tests and future profiling.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueryStats {
    pub parse_runs: usize,
    pub index_and_type_check_runs: usize,
}

/// Memoized semantic query facade.
///
/// This small facade establishes the query boundaries that a future salsa
/// database can adopt directly: parse snapshots first, then index/type-check a
/// workspace snapshot. Cache keys normalize whitespace for the early parser so
/// whitespace-only changes can avoid expensive downstream recomputation.
#[derive(Debug, Default)]
pub struct SemanticQueryDatabase {
    parse_cache: HashMap<QueryKey, SyntaxParse>,
    analysis_cache: HashMap<String, SemanticAnalysis>,
    stats: QueryStats,
}

impl SemanticQueryDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stats(&self) -> QueryStats {
        self.stats
    }

    pub fn parse(&mut self, snapshot: &SourceSnapshot) -> SyntaxParse {
        let key = QueryKey::from_snapshot(snapshot);
        if let Some(parsed) = self.parse_cache.get(&key) {
            return parsed.clone();
        }

        self.stats.parse_runs += 1;
        let parsed = parse_source(&snapshot.text);
        self.parse_cache.insert(key, parsed.clone());
        parsed
    }

    pub fn analyze(&mut self, snapshots: &[SourceSnapshot]) -> SemanticAnalysis {
        for snapshot in snapshots {
            let _ = self.parse(snapshot);
        }

        let key = workspace_key(snapshots);
        if let Some(analysis) = self.analysis_cache.get(&key) {
            return analysis.clone();
        }

        self.stats.index_and_type_check_runs += 1;
        let files: Vec<SourceFile> = snapshots
            .iter()
            .map(|snapshot| SourceFile::new(snapshot.uri.clone(), snapshot.text.clone()))
            .collect();
        let analysis = analyze_workspace(&files);
        self.analysis_cache.insert(key, analysis.clone());
        analysis
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct QueryKey {
    uri: String,
    durability: QueryDurability,
    normalized_text: String,
}

impl QueryKey {
    fn from_snapshot(snapshot: &SourceSnapshot) -> Self {
        Self {
            uri: snapshot.uri.clone(),
            durability: snapshot.durability,
            normalized_text: normalize_for_incremental_key(&snapshot.text),
        }
    }
}

fn workspace_key(snapshots: &[SourceSnapshot]) -> String {
    let mut parts: Vec<String> = snapshots
        .iter()
        .map(|snapshot| {
            format!(
                "{}:{:?}:{}",
                snapshot.uri,
                snapshot.durability,
                normalize_for_incremental_key(&snapshot.text)
            )
        })
        .collect();
    parts.sort();
    parts.join("|")
}

fn normalize_for_incremental_key(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}
