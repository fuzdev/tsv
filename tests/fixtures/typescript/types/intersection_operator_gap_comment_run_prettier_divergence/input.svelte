<script lang="ts">
	// an own-line comment before the `&` and one trailing the `&` both follow the operator,
	// each on its own line, in the order the author wrote them
	type A1 = A &
		// x
		// c
		B;

	// the same with two own-line comments ahead of the operator
	type A2 = A &
		// x1
		// x2
		// c
		B;

	// and as an index signature value
	type A3 = {
		[k: string]: A &
			// x
			// c
			B;
	};

	// a `//` trailing the previous member and one trailing the `&`: each keeps its own line
	type A4 = A & // t
		// c
		B;

	// an own-line block ahead of the operator and a `//` trailing it stay on separate lines
	type A5 = A &
		/* b */
		// c
		B;
</script>
