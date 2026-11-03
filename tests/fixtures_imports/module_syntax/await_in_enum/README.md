An `await` in an enum member initializer is not a top-level one: tsc reads the initializer as function-like and rejects it there (TS1308), so it is no module syntax — `hasModuleSyntax: false`.
