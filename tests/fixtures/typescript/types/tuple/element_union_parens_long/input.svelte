<script lang="ts">
	// the union fits at 100 inside the expanded tuple: it stays on one line and takes no parens
	type Fits = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		d
	];

	// 101: the union breaks, and prettier wraps it in parens so `| b,` shows where the element ends
	type Breaks = [
		(
			| aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		),
		d
	];

	// a ONE-element tuple takes no parens however the union breaks
	type Single = [
		| aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		| bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		| cccccccccccccccccccccccccccccccccccccccccccccc
	];

	// a NAMED member is keyed off the label, not the element list, so it takes none either
	type Named = [
		a:
			| aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		d
	];

	// an intersection has no such rule
	type Intersect = [
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		d
	];

	// the whole tuple inline needs no parens at all
	type Inline = [a | b, d];
</script>
