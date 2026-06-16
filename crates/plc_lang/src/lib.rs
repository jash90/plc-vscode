//! Language-plugin registry and canonical-IR conversion hub for PLC VS Code.
//!
//! IEC 61131-3 defines five languages (ST, IL, LD, FBD, SFC). This crate makes
//! the project ready to add them behind one object-safe plugin trait and to
//! **convert one language into another** through a canonical intermediate
//! representation (`plc_hir::HirModule`):
//!
//! - A [`LanguageFrontend`] *lowers* its source into the IR and *renders* the IR
//!   back into its source. Implement it (plus register it) to add a language.
//! - [`LanguageRegistry::convert`] is the hub: `to.render(from.lower(src))`, so
//!   conversion costs N lowerers + M renderers — not N² pairwise converters.
//!
//! The IR is reused as-is (no `plc_hir` changes), so today's faithful conversion
//! subset is exactly what the IR models: POUs with `VAR` declarations and a body
//! of assignment statements over `Int/Real/Bool/Str/Var` and `+`/`-`. Constructs
//! the IR does not model (control flow, calls, `*`/`/`, comparisons, …) are
//! surfaced as **fidelity notes** — never silently mistranslated. Graphical
//! languages (LD/FBD/SFC) fit the same trait later behind additive IR overlays.

use std::collections::HashMap;
use std::sync::Arc;

use plc_api::{
    Analysis, CodeAction, CompletionCandidate, Diagnostic, DiagnosticSeverity, ExecutionResult,
    HoverInfo, LanguageService, Location, Position, Range, SemanticToken, SignatureInfo,
    SourceDocument, SymbolAnalysis, TextEdit, WorkspaceSymbol,
};
use plc_hir::HirModule;

#[cfg(feature = "il")]
mod il;
#[cfg(feature = "st")]
mod st;

#[cfg(feature = "il")]
pub use il::IlFrontend;
#[cfg(feature = "st")]
pub use st::StFrontend;

/// Result of lowering a language's source into the canonical IR.
#[derive(Debug, Clone)]
pub struct LoweringResult {
    pub module: HirModule,
    pub diagnostics: Vec<Diagnostic>,
    /// Source constructs not represented in the IR (honest partial coverage).
    pub fidelity: Vec<String>,
}

/// Result of rendering the canonical IR into a language's source text.
#[derive(Debug, Clone)]
pub struct RenderResult {
    pub text: String,
    /// IR constructs this language cannot express faithfully.
    pub fidelity: Vec<String>,
}

impl RenderResult {
    /// A renderer that does not (yet) support this language.
    pub fn unsupported(language_id: &str) -> Self {
        Self {
            text: String::new(),
            fidelity: vec![format!("rendering to `{language_id}` is not supported")],
        }
    }
}

/// Why a conversion could not produce faithful output. `convert` never panics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionError {
    UnknownSource(String),
    UnknownTarget(String),
    /// The source had error-level diagnostics; no render was attempted.
    SourceHasErrors,
    /// The target language cannot render the IR (no renderer).
    RenderUnsupported(&'static str),
}

/// Outcome of converting one language into another through the IR hub.
#[derive(Debug, Clone)]
pub struct ConversionResult {
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
    pub fidelity: Vec<String>,
    pub error: Option<ConversionError>,
}

/// The plugin contract for one PLC language.
///
/// Object-safe (held as `Box<dyn LanguageFrontend>` in the registry): all
/// methods take `&self` + borrowed/`Copy` args and return owned values — no
/// generics, no associated types, no `async`, no `Self` by value. The IR type is
/// the concrete [`HirModule`] so `convert` is a trivial composition. `render`,
/// `can_render`, `analyze`, and `language_service` are defaulted, so a minimal or
/// analysis-only (or graphical-later) frontend compiles without them.
pub trait LanguageFrontend: Send + Sync {
    /// Stable language id, e.g. `"st"`, `"il"`.
    fn id(&self) -> &'static str;
    /// Human-readable name, e.g. `"Structured Text"`.
    fn display_name(&self) -> &'static str;
    /// Lowercased file extensions used to select this language, e.g. `["st"]`.
    fn extensions(&self) -> &'static [&'static str];
    /// Parse source into the canonical IR (+ diagnostics + fidelity notes).
    fn lower(&self, document: &SourceDocument) -> LoweringResult;
    /// Whether this language can render the IR back to source.
    fn can_render(&self) -> bool {
        false
    }
    /// Render the canonical IR into this language's source text.
    fn render(&self, _module: &HirModule) -> RenderResult {
        RenderResult::unsupported(self.id())
    }
    /// Diagnostics for IDE consumers. Defaults to lowering diagnostics.
    fn analyze(&self, document: &SourceDocument) -> Analysis {
        let lowered = self.lower(document);
        Analysis::new(
            document.uri().to_owned(),
            document.version(),
            lowered.diagnostics,
        )
    }
    /// A full IDE backend for this language, if any (drives the LSP server).
    /// Languages without rich IDE support return `None` and callers fall back.
    fn language_service(&self) -> Option<Arc<dyn LanguageService + Send + Sync>> {
        None
    }
}

/// Registry of language frontends — the single object LSP/CLI consult to become
/// language-aware. First-registered wins on an extension collision (disambiguate
/// via [`LanguageRegistry::frontend_by_id`]).
#[derive(Default)]
pub struct LanguageRegistry {
    frontends: Vec<Box<dyn LanguageFrontend>>,
    by_id: HashMap<String, usize>,
    by_extension: HashMap<String, usize>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry preloaded with the built-in reference languages (ST, then IL),
    /// according to enabled cargo features.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        #[cfg(feature = "st")]
        registry.register(Box::new(StFrontend));
        #[cfg(feature = "il")]
        registry.register(Box::new(IlFrontend));
        registry
    }

    pub fn register(&mut self, frontend: Box<dyn LanguageFrontend>) {
        let index = self.frontends.len();
        self.by_id.entry(frontend.id().to_owned()).or_insert(index);
        for extension in frontend.extensions() {
            self.by_extension
                .entry(extension.to_ascii_lowercase())
                .or_insert(index);
        }
        self.frontends.push(frontend);
    }

    pub fn frontend_by_id(&self, id: &str) -> Option<&dyn LanguageFrontend> {
        self.by_id
            .get(id)
            .map(|&index| self.frontends[index].as_ref())
    }

    pub fn frontend_for_extension(&self, extension: &str) -> Option<&dyn LanguageFrontend> {
        self.by_extension
            .get(&extension.to_ascii_lowercase())
            .map(|&index| self.frontends[index].as_ref())
    }

    /// Resolve the frontend for a document URI/path by its file extension.
    pub fn frontend_for_uri(&self, uri: &str) -> Option<&dyn LanguageFrontend> {
        let extension = uri.rsplit('.').next()?;
        self.frontend_for_extension(extension)
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.frontends
            .iter()
            .map(|frontend| frontend.id())
            .collect()
    }

    /// Resolve a [`LanguageService`] for a document URI so an LSP host can be
    /// language-aware: the frontend's own rich backend if it has one
    /// (`StFrontend` -> `CompilerCore`), otherwise a diagnostics-only adapter
    /// over the frontend's `analyze` (so e.g. `.il` files still get diagnostics
    /// instead of being analyzed as ST). Returns `None` if no language matches.
    pub fn language_service_for_uri(
        self: &Arc<Self>,
        uri: &str,
    ) -> Option<Arc<dyn LanguageService + Send + Sync>> {
        let frontend = self.frontend_for_uri(uri)?;
        if let Some(service) = frontend.language_service() {
            return Some(service);
        }
        Some(Arc::new(FrontendDiagnostics {
            registry: Arc::clone(self),
            id: frontend.id(),
        }))
    }

    /// Convert `document` from `from_id` to `to_id` through the canonical IR hub.
    /// Never panics: unknown languages, source errors, and unsupported targets
    /// are reported via [`ConversionResult::error`].
    pub fn convert(
        &self,
        from_id: &str,
        to_id: &str,
        document: &SourceDocument,
    ) -> ConversionResult {
        let Some(from) = self.frontend_by_id(from_id) else {
            return ConversionResult {
                text: String::new(),
                diagnostics: Vec::new(),
                fidelity: Vec::new(),
                error: Some(ConversionError::UnknownSource(from_id.to_owned())),
            };
        };
        let Some(to) = self.frontend_by_id(to_id) else {
            return ConversionResult {
                text: String::new(),
                diagnostics: Vec::new(),
                fidelity: Vec::new(),
                error: Some(ConversionError::UnknownTarget(to_id.to_owned())),
            };
        };

        let lowered = from.lower(document);
        if lowered
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            return ConversionResult {
                text: String::new(),
                diagnostics: lowered.diagnostics,
                fidelity: lowered.fidelity,
                error: Some(ConversionError::SourceHasErrors),
            };
        }

        if !to.can_render() {
            return ConversionResult {
                text: String::new(),
                diagnostics: lowered.diagnostics,
                fidelity: lowered.fidelity,
                error: Some(ConversionError::RenderUnsupported(to.id())),
            };
        }

        let rendered = to.render(&lowered.module);
        let mut fidelity = lowered.fidelity;
        fidelity.extend(rendered.fidelity);
        ConversionResult {
            text: rendered.text,
            diagnostics: lowered.diagnostics,
            fidelity,
            error: None,
        }
    }
}

/// Diagnostics-only [`LanguageService`] adapter over a frontend that lacks a
/// rich IDE backend. Diagnostics come from the frontend's `analyze`; every other
/// feature returns empty so the editor degrades gracefully.
struct FrontendDiagnostics {
    registry: Arc<LanguageRegistry>,
    id: &'static str,
}

impl LanguageService for FrontendDiagnostics {
    fn analyze(&self, document: &SourceDocument) -> Analysis {
        match self.registry.frontend_by_id(self.id) {
            Some(frontend) => frontend.analyze(document),
            None => Analysis::new(document.uri().to_owned(), document.version(), Vec::new()),
        }
    }
    fn execute(&self, _: &SourceDocument) -> ExecutionResult {
        ExecutionResult::new(Vec::new(), Vec::new())
    }
    fn document_symbols(&self, document: &SourceDocument) -> SymbolAnalysis {
        SymbolAnalysis::new(document.uri().to_owned(), document.version(), Vec::new())
    }
    fn workspace_symbols(&self, _: &[SourceDocument], _: &str) -> Vec<WorkspaceSymbol> {
        Vec::new()
    }
    fn semantic_tokens(&self, _: &SourceDocument) -> Vec<SemanticToken> {
        Vec::new()
    }
    fn completions(&self, _: &SourceDocument, _: Position) -> Vec<CompletionCandidate> {
        Vec::new()
    }
    fn hover(&self, _: &SourceDocument, _: Position) -> Option<HoverInfo> {
        None
    }
    fn signature_help(&self, _: &SourceDocument, _: Position) -> Option<SignatureInfo> {
        None
    }
    fn definition(&self, _: &SourceDocument, _: Position) -> Option<Location> {
        None
    }
    fn references(&self, _: &SourceDocument, _: Position, _: bool) -> Vec<Location> {
        Vec::new()
    }
    fn formatting(&self, _: &SourceDocument) -> Vec<TextEdit> {
        Vec::new()
    }
    fn formatting_range(&self, _: &SourceDocument, _: Range) -> Vec<TextEdit> {
        Vec::new()
    }
    fn code_actions(&self, _: &SourceDocument) -> Vec<CodeAction> {
        Vec::new()
    }
}

/// Compile-time proof that the plugin trait stays object-safe.
fn _assert_object_safe(_: &dyn LanguageFrontend) {}
