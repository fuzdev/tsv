An `await` in a namespace body is not a top-level one: tsc reads the body as function-like and rejects it there (TS1308), so it is no module syntax — `hasModuleSyntax: false`.
