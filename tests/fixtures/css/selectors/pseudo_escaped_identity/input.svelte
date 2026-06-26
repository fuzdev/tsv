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
</style>
