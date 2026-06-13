use plc_workspace::architecture::{Layer, workspace_manifest};

#[test]
fn workspace_declares_required_product_boundaries() {
    let manifest = workspace_manifest();

    assert!(manifest.has_module("compiler_core"));
    assert!(manifest.has_module("cli"));
    assert!(manifest.has_module("lsp_server"));
    assert!(manifest.has_module("vscode_client"));
}

#[test]
fn compiler_core_is_shared_by_cli_lsp_and_runtime_tools() {
    let manifest = workspace_manifest();

    for consumer in [
        "cli",
        "lsp_server",
        "runtime",
        "bytecode_vm",
        "native_backend",
    ] {
        assert!(
            manifest.depends_on(consumer, "compiler_core"),
            "{consumer} should consume the shared compiler core"
        );
    }
}

#[test]
fn architecture_layers_match_planned_delivery_order() {
    let manifest = workspace_manifest();

    assert_eq!(manifest.layer_of("syntax"), Some(Layer::Syntax));
    assert_eq!(
        manifest.layer_of("semantic_analysis"),
        Some(Layer::Semantics)
    );
    assert_eq!(manifest.layer_of("lsp_server"), Some(Layer::Ide));
    assert_eq!(manifest.layer_of("bytecode_vm"), Some(Layer::Runtime));
    assert_eq!(
        manifest.layer_of("native_backend"),
        Some(Layer::NativeCodegen)
    );
}
