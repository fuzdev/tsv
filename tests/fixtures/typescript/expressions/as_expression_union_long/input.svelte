<script lang="ts">
	// union as-cast at the print-width boundary (100) stays inline
	const a = value as Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// one char over the boundary (101): break after `as`, leading-pipe indented members
	const b = value as
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// satisfies breaks its union the same way
	const c = value satisfies
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbb;

	// breakable call LHS stays inline; the union is what breaks
	const d = race([a1, b1, c1, d1]) as
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbb;
</script>
