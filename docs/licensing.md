# Reference project licensing rules

This project may use public PLC/Structured Text projects as references, but licensing boundaries must stay explicit.

## Allowed reference usage

- Use reference projects to understand terminology, architecture trade-offs, IEC 61131-3 behavior, and test-case ideas.
- Re-implement behavior from specifications or independently written notes.
- Prefer links and citations in design notes over copying source text.
- Keep implementation commits reviewable so copied code cannot enter the repository accidentally.

## IronPLC

IronPLC is recorded as an MIT-licensed reference project.

Allowed use:

- Study parser, diagnostics, project structure, and compatibility decisions.
- Compare behavior against independently written PLC VS Code tests.
- Reference the project in architecture and roadmap documentation.

Required guardrail:

- If any IronPLC source code is copied or adapted, the copied portion must preserve the MIT license notice and must be reviewed explicitly before merging. The preferred path is still independent implementation.

## RuSTy

RuSTy is recorded as architecture inspiration only because of LGPL/GPL licensing risk.

Allowed use:

- Study high-level architecture, feature scope, and design vocabulary.
- Use it as a comparison point in non-code documentation.

Not allowed:

- Do not copy RuSTy code.
- Do not translate RuSTy implementation bodies into this repository.
- Do not import RuSTy source files, generated artifacts, or tests unless legal review approves the exact licensing impact first.

## Review checklist

Before using external reference material in an implementation task:

1. Identify the reference project and license.
2. Confirm whether the task uses ideas/spec behavior or source code.
3. For source-code reuse, document the exact files and license obligations before merging.
4. For RuSTy specifically, stop and redesign from specifications or independently authored tests instead of copying.
