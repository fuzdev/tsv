<style>
	/* Attribute-selector names preserve escapes verbatim in the FORMATTED output (the
	   formatter must not decode them), and the AST `name` is half-decoded like Svelte's
	   read_identifier: hex escapes decode, identity escapes keep the `\`. Mirrors
	   class/id/type (escaped_names) and pseudos (pseudo_escaped_identity). */

	/* hex escape: AST name decodes to "Ab"; formatter keeps the raw source */
	[\41 b] {
		color: red;
	}

	/* identity escape: AST name keeps the backslash ("f\oo"); formatter keeps raw */
	[f\oo='baz'] {
		color: blue;
	}
</style>
