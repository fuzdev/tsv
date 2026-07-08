<script lang="ts">
	// A leading own-line comment on a ternary branch must not drop the branch's
	// clarity parens.

	// Nullish `??` in the alternate
	const a = cond
		? x
		: // c
			y ?? z;

	// Nullish `??` in the consequent
	const b = cond
		? // c
			y ?? z
		: w;

	// `as` cast in the consequent
	const c = cond
		? // c
			x as T
		: w;
</script>
