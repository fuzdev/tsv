<script lang="ts">
	// 100 - the whole expression fits, so nothing breaks
	const a = ((aaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbb) || cccccccccccccc).toString();

	// 101 - the parens break onto their own lines; the operand chain stays flat
	const b = (
		(aaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbb) ||
		ccccccccccccccc
	).toString();

	// an arithmetic base takes the same shape
	const c = (
		(aaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbb) /
		ccccccccccccccccc
	).toString();

	// so does a base mixing `??` with `&&`
	const d = (
		(aaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbb) ??
		ccccccccccccccc
	).toString();

	// the shape does not change with what follows the base - a non-null
	// assertion and an optional access lay out identically
	const e = ((aaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbb) ||
		cccccccccccccc)!.toString();

	const f = (
		(aaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbb) ||
		cccccccccccccc
	)?.toString();

	// a flat chain has no nested operand to hold together, so every operand breaks
	const g = (
		aaaaaaaaaaaaaaaaaaaaaaaaa &&
		bbbbbbbbbbbbbbbbbbbbbbbbb &&
		ccccccccccccccccc
	).toString();
</script>
