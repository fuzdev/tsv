<style>
	/* Identity escapes (a backslash before a non-hex char) are kept verbatim in the
	   selector-name AST, matching Svelte's read_identifier half-decode: hex escapes
	   decode, identity escapes keep the backslash. The formatter preserves the raw
	   source either way. Mirrors class/id/type in escaped_names, now for pseudos. */
	:f\oo {
		color: red;
	}
	::b\gar {
		color: blue;
	}

	/* An identity-escaped `(` is part of the NAME: the function-token's `(` is the
	   NEXT one, so the name runs past the escape rather than stopping at it. */
	:foo\(bar(.x) {
		color: green;
	}

	::baz\((.y) {
		color: teal;
	}
</style>
