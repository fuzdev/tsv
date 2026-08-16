<script>
	// a block comment glued to the frozen member slice's base is printed by the base it
	// binds to, ahead of the verbatim slice

	const a = /* c */ fn(2.0000)
		// prettier-ignore
		.c;

	// a bundler annotation binds its token the same way
	const b = /* @__PURE__ */ fn(2.0000)
		// prettier-ignore
		.c;

	// a multi-line block takes the value onto its own line
	const c =
		/* line1
	line2 */ fn(2.0000)
		// prettier-ignore
		.c;

	// an own-line line comment leads the LINE rather than the token, so the gap emits it
	const d =
		// text
		fn(2.0000)
			// prettier-ignore
			.c;

	// an unfrozen chain keeps the same comment in the same place
	const e = /* c */ fn(2.0).c;
</script>
