<script lang="ts">
	// Prettier never breaks BEFORE a computed `[` (member.js `shouldInline` includes
	// `node.computed`); the brackets break instead. These pin that rule at the width
	// boundary and at the numeric-index carve-out.

	// 100 - base and lookup fit on one line
	const a = (expr as AbcDefGhiJklMnoPqrs)[keyAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa];

	// 101 - one char longer: the BRACKETS break, and the lookup still hugs the `)`
	const b = (expr as AbcDefGhiJklMnoPqrs)[
		keyAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	];

	// An optional lookup follows the same rule
	const c = (expr as AbcDefGhiJklMnoPqrs)?.[
		keyAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	];

	// A numeric index never breaks its brackets (prettier isNumericLiteral carve-out),
	// so it overflows the print width rather than splitting
	const d =
		objectAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa[0];

	// The same base with a non-numeric index DOES break its brackets
	const e =
		objectAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa[
			k
		];
</script>
