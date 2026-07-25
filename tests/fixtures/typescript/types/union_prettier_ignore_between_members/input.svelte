<script lang="ts">
	// an own-line directive between members freezes only the following member;
	// the union stays broken one member per line
	type A =
		| (a1 & a2)
		// prettier-ignore
		| (b1&b2);

	// multiple independent directives each freeze only their own following member
	type B =
		| (a1 & a2)
		// prettier-ignore
		| (b1&b2)
		| (c1 & c2)
		// prettier-ignore
		| (d1&d2);
</script>
