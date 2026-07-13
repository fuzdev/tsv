<script lang="ts">
	// No break point ever precedes a computed lookup, whatever sits between it and the
	// object (prettier member.js `shouldInline`). It sheds width via its own brackets.

	// A non-null assertion between the object and the lookup
	const a = (expr as AbcDefGhiJklMnoPqr)![
		keyAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	];

	// A computed lookup on a computed lookup: neither `[` takes a break
	const b = (expr as AbcDefGhiJklMnoPq)[keyAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa][
		keyBbbbbbbbbbbbbbbbbbbbbb
	];

	// Contrast: a plain `.prop` after a lookup DOES take a break point
	const c = (expr as AbcDefGhiJklMnoPqrs)[keyAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]
		.propertyBbbb;

	// A string-literal index is not the numeric carve-out, so its brackets break
	const d = (expr as AbcDefGhiJklMnoPqrs)[
		'keyAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
	];

	// A non-identifier base is no exception
	const e = `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`[
		keyBbbbbbbb
	];
</script>
