<script lang="ts">
	// an author blank line above an own-line comment after the member is kept, as a type
	// literal keeps one after its last member
	type A = {
		[K in T]: V;


		// c1
	};

	// a block comment answers the same way
	type B = {
		[K in T]: V;

		/* c2 */
	};

	// behind a comment trailing the member, and between two own-line comments
	type C = {
		readonly [K in T]?: V; // c3

		// c4
		// c5


		// c6
	};

	// null control: with no blank line, none is added
	type D = {
		-readonly [K in T as U]-?: V;
		// c7
	};
</script>
