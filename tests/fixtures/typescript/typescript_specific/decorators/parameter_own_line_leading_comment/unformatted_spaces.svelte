<script lang="ts">
	class A {
		// leading comment + own-line decorator: no blank between them
		constructor(
				// c
			@dec
				readonly a
		) {}

		// plain (non-property) decorated param: same rule
		fn1(
			// c
				@dec
			b: number
		) {}

		// inline decorator: comment on its own line, decorator hugs the binding
		fn2(
			// c
			@dec    c: number
		) {}

		// author blank between comment and decorator is preserved
		fn3(
			// c

				@dec
			d: number
		) {}

		// non-first param carries the comment + own-line decorator
		fn4(
			e: number,
				// c
			@dec
				f: number
		) {}

		// multiple own-line decorators: measured to the first decorator
		fn5(
			// c
				@dec1
			@dec2
				g: number
		) {}
	}
</script>
