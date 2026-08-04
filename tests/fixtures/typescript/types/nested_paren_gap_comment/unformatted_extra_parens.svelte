<script lang="ts">
	// Redundant nested parens collapse to the one retained pair, so the whole `(`…`)`
	// region is a single gap - a comment in the outer layer stays inside the parens
	// (unformatted_extra_parens carries the nested authoring)
	type A = [((T | U) /* c1 */)?];
	type B = (((T | U) /* c2 */))[];

	// The leading side of the same gap
	type C = [(/* c3 */ ((T | U)))?];
</script>
