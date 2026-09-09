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

		// A parameter property takes the same rule at both of its gaps
		constructor(
			@fn1

			// c4
			@fn2
			private p: T
		) {}

		// ...and so does a plain parameter, where prettier drops the blank between two
		// decorators and keeps it before the binding
		n(
			@fn1

			// c5
			@fn2
			q: T
		) {}

		// The gap before a plain parameter's BINDING: prettier keeps the blank here, so
		// this cell is a match — the run has no following node to lead, and prettier's
		// fallback then trails it onto the decorator
		o(
			@fn1

			// c8
			r
		) {}

		// The null control: with no comment in the gap there is no blank to carry, so a
		// blank between two decorators collapses — see unformatted_ours_null_control
		@fn1
		@fn2
		c = 1;
	}

	// Class decorator whose comment leads the class
	@fn1

	// c6
	class B {}

	// Class decorator followed by another decorator: the same rule, where prettier drops
	@fn1

	// c7
	@fn2
	class C {}

	// The null control again, at the class level
	@fn1
	@fn2
	class D {}
</script>
