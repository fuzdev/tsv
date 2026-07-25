<script lang="ts">
	// a directive between a parameter's decorator and its binding freezes the binding —
	// the node it precedes — and the decorators stay outside the frozen slice
	class A {
		constructor(
			@dec
			// prettier-ignore
			private a:   {x:   1},
			b: U
		) {}

		m(
			@dec1
			@dec2
			// prettier-ignore
			c:   {x:   1},
			d: U
		) {}
	}
</script>
