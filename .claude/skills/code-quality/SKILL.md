---
name: code-quality
description: "Code quality standards. Defines complexity management, modular design, code smell detection. Applied automatically when writing or reviewing code."
---

# Programming Philosophy and Quality Standards

## Core Philosophy

- Code is primarily written for humans to read and maintain; machine execution is a by-product
- Priority: **Readability & Maintainability > Correctness > Performance > Code length**
- Follow idiomatic practices of each language community

## Complexity Management

```
Complexity = Dependencies + Obscurity
```

### Symptoms to Watch For

| Symptom | Description |
|---------|-------------|
| **Change Amplification** | Small changes require modifications in many places |
| **Cognitive Load** | Developers need excessive information to complete tasks |
| **Unknown Unknowns** | Unclear what code needs modification (worst symptom) |

### Mitigation Strategies

- "Zero tolerance" for incremental complexity growth
- Invest time upfront in design
- Avoid tactical shortcuts that create technical debt

## Code Quality Principles

1. Readability First
  - Code is read much more often than it is written.
  - Clear variable and function names.
  - Prefer self‑documenting code over comments.
  - Consistent formatting.
2. KISS (Keep It Simple, Stupid)
  - Choose the simplest solution that works.
  - Avoid over‑engineering.
  - Do not optimize prematurely.
  - Easier to understand > clever code.
3. DRY (Don’t Repeat Yourself)
  - Extract common logic into functions.
  - Create reusable components.
  - Share utility functions across modules.
  - Avoid copy‑paste programming.
4. YAGNI (You Aren’t Gonna Need It)
  - Do not build features you don’t need yet.
  - Avoid speculative generalization.
  - Add complexity only when required.
  - Start simple, refactor when needed.

## Modular Design Principles

- **Deep Modules**: Powerful functionality through simple interfaces
- **Information Hiding**: Encapsulate design decisions within implementations
- **General-Purpose Design**: Combat over-specialization
- **Avoid "Classitis"**: More classes/components ≠ better design

## Code Smells to Watch For

Proactively identify and flag:
- Duplicated logic / copy-paste code
- Over-tight coupling or circular dependencies
- Fragile designs where one change breaks unrelated parts
- Unclear intent, confused abstractions, vague naming
- Over-engineering without real benefit

When identifying code smells:
- Explain the problem concisely
- Provide 1–2 refactoring directions with pros/cons

## Error Handling Strategy

- **Define errors out of existence** — design APIs with no exceptions when possible
- **Mask exceptions** at low levels to protect higher layers
- **Aggregate exceptions** with general-purpose handlers
- **Just crash** for rare, unrecoverable errors