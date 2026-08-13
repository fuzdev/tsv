<script lang="ts">
	class A {
		// Property decorator: an author blank before the comment run survives
		@fn1

		// c1
		@fn2
		a = 1;

		// The run leads the member itself
		@fn1

		/* c2 */
		b = 1;

		// Method decorator, same rule
		@fn1

		// c3
		@fn2
		m() {}

		// A parameter PROPERTY is property-like, so its decorator gap keeps the blank
		constructor(
			@fn1

			// c4
			@fn2
			private p: T
		) {}

		// A plain parameter is not, so the blank is dropped there
		n(
			@fn1
			// c5
			@fn2
			q: T
		) {}
	}

	// Class decorator whose comment leads the class: the blank survives
	@fn1

	// c6
	class B {}

	// Class decorator followed by another decorator: the blank is dropped
	@fn1
	// c7
	@fn2
	class C {}
</script>
