# Vendored CPDev VMSpec descriptors

These six XML files are the CPDev Virtual Machine *specification* — the symbolic
instruction catalog (opcode `vmcode`s, type codes, operand addressing modes, and
the function-block / inline-macro definitions). They are the authoritative source
the [`spec`](../src/spec.rs) module parses to build the opcode table the codegen
and assembler consult.

`VM-Univ.xml` is the master descriptor; it `INCLUDE`s `VMCore.xml` (PRE) and then
`lreals.xml`, `le-IF.xml`, `flash.xml`, `strings.xml` (POST), in that order.

Provenance: supplied by the project owner (the CPDev toolchain authors,
Politechnika Rzeszowska / Katedra Informatyki i Automatyki). They describe the
same VM whose C++ implementation is vendored under
`crates/plc_cpdev_vm/vendor/cpdev/` (whose `vm/vmspec/vmdef.h` carries the
matching `VMF_*` opcode constants, used as a cross-check in the `spec` tests).

The binary `.XCP` layout these opcodes serialize to is **not** described by the
XML; it was recovered from the vendored VM + the `WeJeStSt` fixture and is locked
down by the byte-exact re-assembly test (`tests/reassemble_fixture.rs`).
