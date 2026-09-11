//! The ECMAScript parse goal symbol (`Script` vs `Module`).
//!
//! The goal is a parse-time input — literally which symbol the grammar starts
//! from (`ParseScript` vs `ParseModule` in the spec). It governs *syntactic
//! availability* of a handful of constructs and nothing else.
//!
//! It is orthogonal to **strictness**, which is a property of the source text
//! rather than of the goal, with one coupling: Module code is strict by
//! definition, while Script code is strict only once a `"use strict"` directive
//! prologue says so. So `Goal` itself toggles the goal-specific grammar below;
//! what strict code disallows is decided separately, by the parser's `strict`
//! state.

/// The syntactic goal symbol a parse runs against.
///
/// Defaults to [`Goal::Module`], which is correct for Svelte `<script>` blocks
/// (always modules) and essentially all real-world TypeScript. [`Goal::Script`]
/// exists for standalone scripts and parser-conformance grading, where the
/// goal-specific constructs differ.
///
/// The only axis this enum moves is the goal symbol. The four constructs that
/// differ between the goals:
///
/// | construct | `Module` | `Script` |
/// | --- | --- | --- |
/// | `await` as an identifier / binding / label / class name | reserved | allowed (`[~Await]`) |
/// | top-level `await` — an *expression*, or a `for await` loop | allowed | syntax error |
/// | `import.meta` | allowed | syntax error |
/// | top-level `import` / `export` *declarations* | allowed | syntax error |
///
/// Dynamic `import(...)` is a call expression valid under both goals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Goal {
    /// `ParseModule` — the default. Top-level `import`/`export`, `import.meta`,
    /// and top-level `await` expressions are available; `await` is reserved as
    /// an identifier. Mirrors acorn's `sourceType: 'module'`.
    #[default]
    Module,
    /// `ParseScript` — `await` is an ordinary identifier (the top level is
    /// `[~Await]`); `import`/`export` declarations, `import.meta`, and top-level
    /// `await` expressions and `for await` loops are syntax errors. Script code is **sloppy** unless its
    /// directive prologue holds a `"use strict"`, so the strict-mode production
    /// disallowances parse where a module rejects them — enumerated once on the flag
    /// that gates them (`Parser::strict`), never re-listed here. The Annex B
    /// web-compatibility grammar is out of scope at both goals.
    Script,
}

impl Goal {
    /// The acorn `sourceType` string this goal serializes to in the public AST
    /// (`Program.sourceType`).
    pub const fn source_type(self) -> &'static str {
        match self {
            Goal::Module => "module",
            Goal::Script => "script",
        }
    }

    /// Parse a goal from its [`source_type`](Goal::source_type) string
    /// (`"module"` / `"script"`), the inverse of `source_type`. Returns `None`
    /// for any other string. The single source of truth for the goal vocabulary,
    /// shared by the CLI `--source-type` flag (`tsv_cli`), the WASM bindings (`tsv_wasm`),
    /// and the fixture goal-marker reader (`tsv_debug`); callers layer their own
    /// default and error formatting on top.
    pub fn from_source_type(s: &str) -> Option<Goal> {
        match s {
            "module" => Some(Goal::Module),
            "script" => Some(Goal::Script),
            _ => None,
        }
    }

    /// The goal a file's **extension** settles, or `None` when it settles nothing.
    ///
    /// Two of the six extensions tsv formats carry a goal in the name rather than in
    /// the file: `.mjs` and `.mts` are ES modules whatever any config says — Node
    /// loads a `.mjs` as ESM unconditionally, and TypeScript maps `.mts`/`.mjs` to
    /// `ModuleKind.ESNext` with the extension overriding `module`
    /// (`src/compiler/program.ts`'s implied-format switch). Their `.c*` counterparts
    /// are the CommonJS half of that same switch, but CommonJS is not the `Script`
    /// goal in any useful sense here — a `.cjs` is script *code* yet nothing in tsv's
    /// output turns on it — so they stay unsettled with `.js`/`.ts`.
    ///
    /// A caller that formats a path reads this instead of naming nothing at all: the
    /// module-then-script fallback exists to reach a **legacy sloppy script**, and a
    /// file whose own extension forbids being one has nothing to fall back to. Every
    /// source the module grammar accepts formats identically either way — the retry
    /// fires only on a module parse *failure* — so this narrows the fallback to the
    /// files it was written for and never changes an output.
    ///
    /// `None` for every other extension (including a Svelte or CSS path, whose parser
    /// has no goal axis at all), leaving the caller's own default in force.
    pub fn from_extension(path: &str) -> Option<Goal> {
        if path.ends_with(".mjs") || path.ends_with(".mts") {
            Some(Goal::Module)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Goal;

    #[test]
    fn source_type_round_trips() {
        for goal in [Goal::Module, Goal::Script] {
            assert_eq!(Goal::from_source_type(goal.source_type()), Some(goal));
        }
    }

    #[test]
    fn from_source_type_rejects_unknown() {
        assert_eq!(Goal::from_source_type("sloppy"), None);
        assert_eq!(Goal::from_source_type("Module"), None); // case-sensitive
        assert_eq!(Goal::from_source_type(""), None);
    }

    #[test]
    fn default_is_module() {
        assert_eq!(Goal::default(), Goal::Module);
    }

    #[test]
    fn only_the_module_only_extensions_settle_a_goal() {
        for path in ["a.mjs", "a.mts", "deep/dir/a.mjs", "a.config.mjs"] {
            assert_eq!(
                Goal::from_extension(path),
                Some(Goal::Module),
                "{path} is an ES module by its own extension"
            );
        }
        for path in [
            "a.js", "a.mjsx", "a.ts", "a.cjs", "a.cts", "a.svelte", "a.css", "amjs", "a", "",
        ] {
            assert_eq!(
                Goal::from_extension(path),
                None,
                "{path} settles no goal — the caller's default stands"
            );
        }
    }
}
