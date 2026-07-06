<script lang="ts">
	const type = 1;
	const as = 2;
	const Foo = 3;
	const g = 4;
	const H = 5;

	// `type` then `as <name>` (one `as`) is a VALUE export of the local `type`,
	// renamed — NOT the type-only modifier.
	export { type as age };

	// `type as` alone is a TYPE-only export of the local `as`.
	export { type as };

	// Two `as` then a name: `type` is the modifier, `as` the exported local,
	// renamed — a TYPE export.
	export { type as as year };

	// `type <name>` is the type-only modifier.
	export { type Foo };

	// Bare `type` is a value export of the local `type`.
	export { type };

	// A type-only modifier on a later specifier (not the first).
	export { g, type H };
</script>
