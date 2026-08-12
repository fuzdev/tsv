<script lang="ts">
	// A member gap that defers a run past the `;` ends the member's doc on a later line,
	// so a comment trailing that `;` takes the line below, distinct and in source order.
	class C1 {
		a = 1; // c1
		/* c2 */ // c3
		b = 2;
	}

	// The same in an interface body.
	interface I1 {
		a: A; // c1
		/* c2 */ // c3
		b: B;
	}

	// And in a type literal.
	type T1 = {
		a: A; // c1
		/* c2 */ // c3
		b: B;
	};
</script>
