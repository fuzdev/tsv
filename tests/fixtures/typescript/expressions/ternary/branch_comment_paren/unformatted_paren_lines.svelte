<script lang="ts">
	// A leading own-line comment on a ternary branch must not drop the branch's
	// clarity parens.

	// Nullish `??` in the alternate
	const a = cond ? x : (
		// c
		y ?? z
	);

	// Nullish `??` in the consequent
	const b = cond ? (
		// c
		y ?? z
	) : w;

	// `as` cast in the consequent
	const c = cond ? (
		// c
		x as T
	) : w;

	// A trailing comment beside the leading one survives too — in the alternate,
	// where the paren shell is the construct the trailing comment sits inside
	const d = cond ? x : (
		// c
		y ?? z // t
	);
</script>
