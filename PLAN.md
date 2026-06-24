# PLC-105: Ladder Diagram (LD) — graficzny edytor, kompilacja LD→ST przez HIR, wykonanie + wizualizacja power-flow

## Kontekst

Projekt PLC VS Code to workspace Rust (11 crate'ów) z pełnym pipeline'em ST. Architektura `plc_lang` (LanguageFrontend trait + IR hub) explicite przewiduje LD jako kolejny język graficzny ("Graphical languages fit the same trait later behind additive IR overlays"). Test `conversion_st_il.rs` już testuje konwersję do `"ld"` oczekując `UnknownTarget("ld")`.

Runtime (`plc_runtime::interp`) ma już pełny AST z `BinOp::{Or,Xor,And,Eq,Ne,Lt,Le,Gt,Ge,Add,Sub,Mul,Div,Mod,Pow}` i `UnOp::{Not,Neg}`, timery (TON/TOF/TP), liczniki (CTU/CTD/CTUD), detektory zboczy (RTrig/FTrig). Ale kanoniczny HIR (`plc_hir`) modeluje tylko `Add`/`Sub` — wymaga rozszerzenia.

**Cel:** Osobny moduł LD — interaktywny edytor graficzny (Webview), pełne LD IEC 61131-3, kompilacja LD→ST przez rozszerzony HIR, wykonanie przez istniejący runtime, wizualizacja power-flow (zielone/czerwone kontakty) na żywo.

## Podejście

```
┌─────────────────────────────────────────────────────┐
│  VS Code Webview (Canvas/SVG)                        │
│  • Rung editor: drag/drop NO, NC, coils, SET/RESET   │
│  • Series (AND) / parallel (OR) connections          │
│  • Timer/Counter/Edge FB blocks                      │
│  • Live power-flow coloring (green=energized)        │
│  • Outputs JSON model on edit                        │
└───────────────┬─────────────────────────────────────┘
                │ JSON model (LdProgram)
                ▼
┌─────────────────────────────────────────────────────┐
│  plc_ld (Rust crate)                                 │
│  • LdProgram model (serde JSON)                      │
│  • LdFrontend: lower() → HirModule (rozszerzony)     │
│  • render() HirModule → ST text                      │
│  • power_flow(): evaluate → LdProgram z energized    │
└───────────────┬─────────────────────────────────────┘
                │ ST source (via StFrontend.render or direct)
                ▼
┌─────────────────────────────────────────────────────┐
│  plc_runtime (istniejący)                            │
│  • Runtime::from_source(st_text)                     │
│  • run_scans(N) → watch() = variable values          │
│  • Wartości wracają do Webview dla power-flow        │
└─────────────────────────────────────────────────────┘
```

### Kluczowe decyzje projektowe

1. **Nowy crate `plc_ld`** — osobny moduł (model LD, serializacja JSON, logic resolver, power-flow). Nie modyfikuje `plc_hir` niemożliwie — tylko dodaje warianty.

2. **HIR rozszerzony** — `BinaryOp` dostaje `And`, `Or`, `Xor`, `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`, `Mul`, `Div`, `Mod`; `HirExpr` dostaje `Unary { op: UnaryOp, .. }` z `UnaryOp::{Not, Neg}`. `HirAssign` nie wystarcza dla LD (SET/RESET coil, FB calls) — dodajemy `HirStmt` z wariantami.

3. **LD → ST przez HIR** — `LdFrontend.lower()` buduje `HirModule` z logicznymi wyrażeniami. `StFrontend.render()` (w `plc_lang`) jest rozszerzane o nowe operatory. To umożliwia `registry.convert("ld", "st", ...)`.

4. **CLI** — `plc ld <file.ld>` kompiluje i wykonuje; `plc ld --watch <file.ld>` podaje power-flow jako JSON dla Webview.

5. **Webview** — Custom editor dla `.ld` plików. Canvas/SVG rysowanie rungów. Po każdej edycji → kompilacja → wykonanie → kolorowanie elementów.

## Pliki do modyfikacji / utworzenia

### Nowy crate: `crates/plc_ld/`
- `Cargo.toml` — zależności: `plc_hir`, `plc_api`, `serde`, `serde_json`
- `src/lib.rs` — re-export API
- `src/model.rs` — `LdProgram`, `Rung`, `Element`, `Contact`, `Coil`, `Block`, `Connection`
- `src/serde.rs` — JSON (de)serializacja modelu LD
- `src/lower.rs` — `LdProgram → HirModule` (logika drabinkowa → HIR wyrażenia logiczne)
- `src/render.rs` — nie używamy bezpośrednio; `LdFrontend` korzysta z `StFrontend::render` przez IR hub
- `src/power_flow.rs` — ocenia które elementy są zasilane (dla wizualizacji)
- `tests/model.rs` — testy modelu
- `tests/lower_to_st.rs` — testy LD→ST przez HIR
- `tests/power_flow.rs` — testy power-flow

### Modyfikacje istniejących crate'ów

#### `crates/plc_hir/src/lib.rs`
- Rozszerzyć `BinaryOp` o: `And`, `Or`, `Xor`, `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`, `Mul`, `Div`, `Mod`
- Dodać `UnaryOp { Not, Neg }`
- Dodać `HirExpr::Unary { op: UnaryOp, expr: Box<HirExpr> }`
- Dodać `HirExpr::Call { name: String, args: Vec<HirExpr> }` (dla timerów/liczników)
- Dodać `HirStmt` z wariantami: `Assign`, `Set`, `Reset`, `FbCall` (dla bloków LD)

#### `crates/plc_hir/src/lib.rs` — `lower_expression`
- Rozszerzyć o rozpoznawanie `AND`/`OR`/`XOR`/`NOT`/`*`/`/`/comparisons w ST tekście (dla round-trip ST→HIR→ST)

#### `crates/plc_lang/src/st.rs` — `render_structured_text` / `render_expr`
- Rozszerzyć `render_expr` o nowe operatory: `And → "AND"`, `Or → "OR"`, itd.
- Rozszerzyć o renderowanie `HirStmt::Set/Reset/FbCall`

#### `crates/plc_lang/src/lib.rs`
- Dodać `#[cfg(feature = "ld")] mod ld;`
- Zarejestrować `LdFrontend` w `with_builtins()` (gated feature `ld`)

#### `crates/plc_lang/Cargo.toml`
- Dodać feature `ld = ["dep:plc_ld"]`
- Dodać `plc_ld = { path = "../plc_ld", optional = true }`
- Dodać `"ld"` do `default`

#### `crates/plc_lang/src/ld.rs` (nowy)
- `LdFrontend` impl `LanguageFrontend` — `lower()` deleguje do `plc_ld`, `render()` używa StFrontend przez IR

#### `crates/plc_lang/tests/conversion_ld.rs` (nowy)
- LD→ST conversion tests przez IR hub
- Round-trip LD→ST→LD (gdzie możliwe)

#### `Cargo.toml` (workspace root)
- Dodać `"crates/plc_ld"` do `members`

#### `editors/vscode/package.json`
- Dodać język `ladder-diagram`, rozszerzenie `.ld`
- Dodać komendy `plc-vscode.openLdEditor`, `plc-vscode.runLd`
- Dodać custom editor contribution dla `.ld`

#### `editors/vscode/src/ldEditor.ts` (nowy)
- Custom editor provider (read/write `.ld` JSON)
- Webview z Canvas/SVG
- Drag/drop elementów, seria/równoległość
- Integracja power-flow

#### `editors/vscode/src/ldWebview.ts` (nowy)
- HTML/JS dla webview: rysowanie rungów, elementów, interakcja

#### `editors/vscode/src/extension.ts`
- Zarejestrować custom editor provider
- Zarejestrować nowe komendy

#### `crates/plc_cli/src/` — `ld` subcommand
- `plc ld <file>` — kompiluj LD→ST, wykonaj, pokaż wyniki
- `plc ld --watch <file>` — wydrukuj power-flow JSON

## Kroki (TDD — każdy krok: test → implementacja → green)

### Faza 1: Model LD + HIR rozszerzenie (backend)

- [ ] **1.1** Rozszerzyć `plc_hir`: `BinaryOp` (And, Or, Xor, Eq, Ne, Lt, Le, Gt, Ge, Mul, Div, Mod), `UnaryOp` (Not, Neg), `HirExpr::Unary`, `HirExpr::Call`, `HirStmt`. Testy: round-trip nowych operatorów w `lower_expression`.
- [ ] **1.2** Rozszerzyć `render_expr` w `plc_lang/src/st.rs` o nowe operatory. Test: `And → "AND"`, `Not → "NOT"`, `Eq → "="`, itd.
- [ ] **1.3** Utworzyć crate `plc_ld` z `Cargo.toml`, dodać do workspace.
- [ ] **1.4** Zaimplementować `model.rs` — `LdProgram { name, rungs }`, `Rung { elements }`, elementy: `Contact { name, negated }`, `Coil { name, kind: Normal|Set|Reset }`, `Block { fb_type, instance, inputs, outputs }`, `Connection` (seria/równoległość). Test: serializacja/deserializacja JSON.
- [ ] **1.5** Zaimplementować `lower.rs` — `LdProgram → HirModule`. Logika: kontakt szeregowy = AND, równoległy = OR, NC = NOT, cewka = Assign. Test: prosta drabinka `(A AND B) → C` loweruje do `C := A AND B`.
- [ ] **1.6** Zaimplementować `power_flow.rs` — evaluacja stanu zmiennych → które elementy zasilone. Test: kontakty z True→zasilone, cewki z True→zasilone.

### Faza 2: LD Frontend + konwersja LD→ST

- [ ] **2.1** Zaimplementować `plc_lang/src/ld.rs` — `LdFrontend` z `lower()` (delegacja do `plc_ld::lower`), `can_render() = false` (renderowanie przez ST). Dodać feature `ld`, zarejestrować w `with_builtins()`.
- [ ] **2.2** Test: `registry.convert("ld", "st", &doc)` produkuje poprawne ST z `AND`/`OR`/`NOT`.
- [ ] **2.3** Test: round-trip LD→ST→ST exec → poprawne wyniki przez `plc_runtime`.
- [ ] **2.4** Rozszerzyć konwersję o timery/liczniki: `Block { fb_type: "TON", .. }` → ST `TON_inst(IN := ..., PT := ...);` + `HIR::FbCall`.
- [ ] **2.5** Rozszerzyć o SET/RESET coils → `HirStmt::Set/Reset`.
- [ ] **2.6** Pełne LD: skoki (JMP), bloki funkcyjne, wszystkie warianty IEC 61131-3.

### Faza 3: CLI

- [ ] **3.1** Dodać `plc ld <file>` do `plc_cli` — kompiluj LD→ST, wykonaj przez runtime, wyświetl watch.
- [ ] **3.2** Dodać `plc ld --watch <file>` — zwróć power-flow JSON.

### Faza 4: VS Code Webview editor

- [ ] **4.1** Dodać custom editor contribution w `package.json` (język `.ld`, komendy).
- [ ] **4.2** Zaimplementować `LdEditorProvider` (custom editor — czyta/zapisuje `.ld` JSON).
- [ ] **4.3** Webview: Canvas/SVG rysowanie rungów — kontakty (NO `| |`, NC `|/|`), cewki `( )`, bloki.
- [ ] **4.4** Webview: drag/drop elementów z palety, łączenie szeregowo/równolegle.
- [ ] **4.5** Webview: po edycji → JSON → CLI `plc ld` → power-flow JSON → kolorowanie (zielony=zasilony, szary=niezasilony).
- [ ] **4.6** Webview: paleta elementów (NO/NC contact, normal/SET/RESET coil, TON/TOF/TP, CTU/CTD, R_TRIG/F_TRIG).

### Faza 5: Integracja + dokumentacja

- [ ] **5.1** Zaktualizować test `conversion_st_il.rs` — `ld` jest teraz zarejestrowane (zmiana oczekiwanego błędu).
- [ ] **5.2** Dodać `crates/plc_ld/tests/full_ld_program.rs` — kompletny program LD z timerami i licznikami, wykonanie, power-flow.
- [ ] **5.3** Dodać fixture `.ld` pliki do `tests/`.
- [ ] **5.4** Zaktualizować `docs/architecture/` — opis modułu LD.

## Reuse (istniejący kod)

| Co | Gdzie | Jak |
|---|---|---|
| `LanguageFrontend` trait | `plc_lang/src/lib.rs` | `LdFrontend` implementuje trait |
| `LanguageRegistry::convert` | `plc_lang/src/lib.rs` | LD→ST przez IR hub (zero zmian) |
| `HirModule`, `HirProgram`, `HirAssign` | `plc_hir/src/lib.rs` | Rozszerzamy, nie zastępujemy |
| `StFrontend::render` | `plc_lang/src/st.rs` | Rozszerzamy `render_expr` o nowe operatory |
| `Runtime::from_source` + `run_scans` | `plc_runtime/src/lib.rs` | LD→ST→Runtime bez zmian |
| Timer/Counter FBs (TON/TOF/TP/CTU/CTD/CTUD) | `plc_runtime/src/timers.rs`, `counters.rs` | LD bloki mapują do tych FB |
| `resolveRunInvocation` | `editors/vscode/src/extension.ts` | CLI `plc ld` używa tego samego patternu |
| `ScanRuntimeEngine` | `plc_runtime/src/engine.rs` | ExecutionEngine dla LD przez ST |

## Weryfikacja

1. **Unit testy (Rust):** `cargo test -p plc_hir` — nowe operatory; `cargo test -p plc_ld` — model, lower, power_flow; `cargo test -p plc_lang --features "st il ld"` — konwersja LD→ST.
2. **Integration testy:** `cargo test --workspace` — całość workspace przechodzi; `cargo test -p plc_cli` — `plc ld` komenda.
3. **Manual VS Code:** Otwórz `.ld` plik → editor graficzny; dodaj kontakty/cewki → automatyczna kompilacja; ustaw inputy → power-flow koloruje elementy; uruchom scan cycles → live update.
4. **LD→ST→exec:** Stwórz LD program z `A AND NOT B → C`; kompiluj do ST (`C := A AND NOT B`); wykonaj 25 scan cycles; sprawdź `C = TRUE` gdy `A=TRUE, B=FALSE`.
5. **Timer test:** LD z `TON_inst(IN := Start, PT := T#2s) → Done`; wykonaj; sprawdź `Done = TRUE` po 2s virtual time.
6. **Test istniejący:** `conversion_st_il.rs` — zaktualizować assertion (LD teraz zarejestrowane).
