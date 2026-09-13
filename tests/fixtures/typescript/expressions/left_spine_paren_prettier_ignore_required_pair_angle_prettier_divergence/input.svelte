<script lang="ts">
	// `-` is accepted bare by every parser and REBINDS at every one: `new DD<T> - 1` reads as
	// `((new DD) < T) > -1`, so the pair goes around the slice to keep the input's tree
	const dd =
		// prettier-ignore
		(new DD<T>) - 1;

	// `<` and `>=` continue the type arguments' `>` at the parser tsc and prettier hold to, so
	// a bare `new DD<T> < 1` is `'>' expected` there and the pair goes around the slice
	const ee =
		// prettier-ignore
		(new DD<T>) < 1;
	const ff =
		// prettier-ignore
		(new DD<T>) >= 1;

	// `<<` is the mirror: tsc reads the bare form back as the same tree and acorn-typescript,
	// whose wire tsv is a drop-in for, rejects it
	const hh =
		// prettier-ignore
		(new DD<T>) << 1;

	// `>`, `>>` and `>>>` are rejected bare by all three parsers
	const ii =
		// prettier-ignore
		(new DD<T>) > 1;
	const jj =
		// prettier-ignore
		(new DD<T>) >> 1;
	const kk =
		// prettier-ignore
		(new DD<T>) >>> 1;

	// `<=` opens no continuation at any parser, so the parse backtracks to the type arguments
	// and the bare form is the same tree — no pair
	const gg =
		// prettier-ignore
		new DD<T> <= 1;
</script>
