<script lang="ts">
	// an own-line directive above a union-valued tuple element freezes the whole element
	type A = [
		// prettier-ignore
		{a:   1} | {b:   2},
		C
	];

	// sole type argument: the whole argument freezes
	type B = G<
		// prettier-ignore
		{a:   1} | {b:   2}
	>;
</script>
