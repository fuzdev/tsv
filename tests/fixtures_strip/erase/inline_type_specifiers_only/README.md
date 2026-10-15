# inline_type_specifiers_only

An import whose braces hold only inline `type` specifiers keeps the declaration, braces emptied (`import {} from './a'`), so the module still loads — TypeScript's `verbatimModuleSyntax` semantics, and both oracles'. Eliding it is an open design question; this fixture pins the oracle form.
