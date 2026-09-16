<script lang="ts">
	// A relational chain whose right operand is an object literal keeps the same paren
	// pair the plain chain does, and for the sharper reason: to tsv, acorn and tsc alike, a
	// line terminator after the `>` commits a type-argument list ahead of an object, so any
	// break there re-lexes the chain into an instantiation plus a free-standing block — and a
	// break arrives from the object's own forced break as readily as from width.

	// The blank line inside the object forces the break at every width.
	const c1 =
		x <
		y >
		{
			a: 1,

			b: 2
		};

	// With the pair, the continuation line below is exactly 100 and the object stays welded.
	const c2 =
		x < y > { kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk: 1, lllllllllllllllllllllllllllllllllllll: 2 };

	// One character longer in the first key than `c2`: with the pair, the welded form no
	// longer fits, so the object drops to its own line — the break the pair survives.
	const c3 =
		x < y > { kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk: 1, lllllllllllllllllllllllllllllllllllll: 2 };
</script>
