# Type params paren comment

Block comments between a type-parameter list's `>` and the opening `(` in
function-likes: function declarations, async functions, function expressions,
class methods (plain, async, generator), and object methods.

tsv matches Svelte's attachment in every case: `trailingComments` on the
`TSTypeParameterDeclaration` for function/method declarations (type params hang
off that node), `leadingComments` on the value `FunctionExpression` for object
methods (type params live on the function expression). The comment is also in
the root `comments` array.

These all have a body, so prettier preserves the comment between `>` and `(`.
The body-less overload/abstract forms, where prettier relocates it inside the
parens, live in `overload_type_params_paren_comment_prettier_divergence`.
