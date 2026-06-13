//! Workspace architecture contract for PLC VS Code.
//!
//! The crate intentionally contains a small executable model of the planned
//! repository boundaries. Tests use it as a living contract while the concrete
//! crates are introduced task-by-task.

pub mod architecture {
    /// High-level delivery and dependency layer for repository modules.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Layer {
        Foundation,
        Syntax,
        Semantics,
        Ide,
        Runtime,
        NativeCodegen,
        Client,
    }

    /// A planned workspace module or package boundary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Module {
        pub name: &'static str,
        pub path: &'static str,
        pub layer: Layer,
        pub depends_on: &'static [&'static str],
        pub responsibility: &'static str,
    }

    /// Static manifest for the planned PLC VS Code repository architecture.
    #[derive(Debug, Clone, Copy)]
    pub struct WorkspaceManifest {
        modules: &'static [Module],
    }

    impl WorkspaceManifest {
        pub fn modules(&self) -> &'static [Module] {
            self.modules
        }

        pub fn has_module(&self, name: &str) -> bool {
            self.modules.iter().any(|module| module.name == name)
        }

        pub fn layer_of(&self, name: &str) -> Option<Layer> {
            self.modules
                .iter()
                .find(|module| module.name == name)
                .map(|module| module.layer)
        }

        pub fn depends_on(&self, module_name: &str, dependency_name: &str) -> bool {
            self.modules
                .iter()
                .find(|module| module.name == module_name)
                .is_some_and(|module| module.depends_on.contains(&dependency_name))
        }
    }

    const MODULES: &[Module] = &[
        Module {
            name: "compiler_core",
            path: "crates/plc_compiler_core",
            layer: Layer::Foundation,
            depends_on: &["syntax", "semantic_analysis"],
            responsibility: "Shared compiler API consumed by CLI, LSP, runtime, and backends.",
        },
        Module {
            name: "syntax",
            path: "crates/plc_syntax",
            layer: Layer::Syntax,
            depends_on: &[],
            responsibility: "Lexer, error-tolerant parser, CST, and source ranges.",
        },
        Module {
            name: "semantic_analysis",
            path: "crates/plc_semantics",
            layer: Layer::Semantics,
            depends_on: &["syntax"],
            responsibility: "Symbol index, name resolution, type model, and diagnostics.",
        },
        Module {
            name: "cli",
            path: "crates/plc_cli",
            layer: Layer::Foundation,
            depends_on: &["compiler_core"],
            responsibility: "Command-line interface for parsing, diagnostics, execution, and compilation.",
        },
        Module {
            name: "lsp_server",
            path: "crates/plc_lsp_server",
            layer: Layer::Ide,
            depends_on: &["compiler_core"],
            responsibility: "Language Server Protocol implementation for Structured Text IDE features.",
        },
        Module {
            name: "runtime",
            path: "crates/plc_runtime",
            layer: Layer::Runtime,
            depends_on: &["compiler_core"],
            responsibility: "PLC scan-cycle execution model, state inspection, and deterministic simulation.",
        },
        Module {
            name: "bytecode_vm",
            path: "crates/plc_bytecode_vm",
            layer: Layer::Runtime,
            depends_on: &["compiler_core", "runtime"],
            responsibility: "Portable bytecode representation and interpreter/VM execution.",
        },
        Module {
            name: "native_backend",
            path: "crates/plc_native_backend",
            layer: Layer::NativeCodegen,
            depends_on: &["compiler_core"],
            responsibility: "Native backend strategy, initially LLVM IR via inkwell.",
        },
        Module {
            name: "vscode_client",
            path: "editors/vscode",
            layer: Layer::Client,
            depends_on: &["lsp_server"],
            responsibility: "TypeScript VS Code extension client and packaging.",
        },
    ];

    pub fn workspace_manifest() -> WorkspaceManifest {
        WorkspaceManifest { modules: MODULES }
    }
}
