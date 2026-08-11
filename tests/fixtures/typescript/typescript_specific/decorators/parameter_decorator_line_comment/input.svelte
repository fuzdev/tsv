<script lang="ts">
	class A {
		// a line comment trailing a parameter decorator is printed once, by that
		// decorator — whatever the author glued the decorator itself to
		fn1(
			@dec // c
			a: T
		) {}

		// between two decorators
		fn2(
			@dec1 // c
			@dec2
			b: T
		) {}

		// after the previous parameter's comma
		fn3(
			a: T,
			@dec // c
			b: T
		) {}

		// on a parameter property's decorator
		constructor(
			@dec // c
			private c: T
		) {}

		// on a destructured parameter's decorator (acorn stores it on the pattern)
		fn4(
			@dec // c
			{ d }: T
		) {}

		// on a default parameter's decorator (stored on the AssignmentPattern)
		fn5(
			@dec // c
			e = 1
		) {}

		// inside the decorator's own arguments
		fn6(
			@dec(
				1 // c
			)
			f: T
		) {}

		// the other side of the seam: a comment the author left on the previous
		// parameter's comma trails that parameter, not the decorator after it
		fn7(
			g: T, // c
			@dec h: T
		) {}
	}
</script>
