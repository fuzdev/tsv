<style>
	div {
		/* a `/` with nothing before it binds to the member after it — but not onto a `*`:
		`/*` is where a comment starts (css-syntax-3 §4.3.2), so that one glue rewrites the
		author's tokens into a comment that never closes, and the gap stands instead */
		a: / * 2.5;
		b: f(/ * a);
		--c: / * 2.5;
		/* the `font` shorthand's own arm keeps a `/` glued to the font-size before it,
		and carries that glue onto the member after it — so it reaches the same merge,
		and takes the same refusal (a custom property rides the same carve-out) */
		font: 12px/ * 2;
		--d: 12px/ *;
	}
</style>
