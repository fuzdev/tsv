<script lang="ts">
	// A `>>`/`>>>` shift after a would-be `<...>` close keeps `<` a relational
	// operator: `a < b >> c` parses as `a < (b >> c)`, a comparison — not the
	// instantiation expression `(a<b>) > c`. Matches acorn: a shift/`>` token
	// right after the close disqualifies the type-argument reading.
	const shift = a < b >> c;
	const unsigned = a < b >>> c;

	// Contrast: a genuine type-argument call still parses as instantiation.
	const call = fn<T>();
</script>
