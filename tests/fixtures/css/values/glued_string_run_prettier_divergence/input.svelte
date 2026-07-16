<style>
	.a {
		/* Two directly-adjacent quoted strings — one opaque run, kept verbatim. */
		content: 'a''b';
		/* String, ident, string glued into one run — kept verbatim. */
		content: 'a'x'b';
		/* Double-quoted run — not re-quoted to a single string. */
		content: "a"x"b";
	}
</style>
