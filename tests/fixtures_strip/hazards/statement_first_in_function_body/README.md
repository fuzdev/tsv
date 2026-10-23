# statement_first_in_function_body

A blanked statement opening a function body — a declaration's, an arrow's, a method's — takes no `;`, even when the JS before the body (the statement `let a = b`, the parameters `c`, `e`, `h`) does not end in one: nothing before a function body's `{` can run on into its first statement, so the `;` state is cleared on entering the body.
