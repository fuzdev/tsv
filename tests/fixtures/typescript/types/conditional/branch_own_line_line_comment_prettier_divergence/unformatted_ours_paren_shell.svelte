<script lang="ts">
	// An own-line `//` in a conditional type's `?`→true-branch gap keeps the line the author
	// gave it; the branch hangs one level in below it.
	type A = B extends C
		?
			// c1
			D
		: E;

	// ...and in the `:`→false-branch gap.
	type F = B extends C
		? D
		:
			// c2
			E;

	// A run keeps one comment per line, in order.
	type G = B extends C
		? D
		:
			// c3
			// c4
			E;

	// A nested conditional's check type and an intersection head hang the same way.
	type H = B extends C
		? (
				// c5
				D
			) extends E ? F : G
		: I;

	type J = B extends C
		? D
		: (
				// c6
				E
			) & F;

	// A chained conditional's false-branch gap.
	type K = B extends C
		? D
		: B extends E
			?
				// c7
				F
			: G;

	// Control: a comment the author put ON the operator's line trails it, as before.
	type L = B extends C
		? // c8
			D
		: E;
</script>
