<script lang="ts">
	// union as-cast at the print-width boundary (100) stays inline
	const a = value as Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// one char over the boundary (101): break after `as`, union stays inline on the next line
	const b = value as Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// satisfies breaks its union the same way
	const c = value satisfies Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbb;

	// breakable call LHS stays inline; the union is what breaks
	const d = race([a1, b1, c1, d1]) as Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbb;

	// union pushed to the next line fits at exactly 100: stays inline, no leading pipes
	const e = value as Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// union pushed to the next line hits 101: members fully expand, one leading-pipe per line
	const f = value as Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// satisfies with more members: same full leading-pipe expansion
	const g = value satisfies Aaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbb | Cccccccccccccccccccccccc | Dddddddddddddddddddddddd;
</script>
