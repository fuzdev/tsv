<script lang="ts">
	// a directive between a parameter's decorator and its binding freezes the binding —
	// the node it precedes — and the decorators stay outside the frozen slice
	class A {
		constructor(
			@dec
			// prettier-ignore
			private a: { x: 1 },
			b: U
		) {}

		m(
			@dec1
			@dec2
			c // prettier-ignore
			:   {x:   1},
			d: U
		) {}

		// the frozen slice prints the binding's source verbatim, redundant parens and the
		// comment inside them included, so the parameter list's own trailing run claims
		// nothing there and the comment is printed once
		n(@dec
			// prettier-ignore
			e = (1   /* c */), f: U) {}
	}
</script>
