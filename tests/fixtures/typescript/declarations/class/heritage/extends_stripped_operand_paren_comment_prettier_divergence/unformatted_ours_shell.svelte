<script lang="ts">
	// a comment written last inside a redundant pair down the right edge of a superclass whose
	// required pair the printer emits stays after the pair, ahead of a comment written after
	// the inner pair, each on a line of its own
	class A1 extends (a + (b // c1
	) // d1
	) {}
	class A2 extends (a + (b // c2
	) /* x2 */ // d2
	) {
		x = 1;
	}
	k3 = class extends (a + (b // c3
	) // d3
	) {};
	class A4 extends (a + (b
	// c4
	) // d4
	) implements I {}

	// with nothing written after the pair, the comment stays after the pair, ahead of the body
	class A5 extends (a + (b // c5
	)) {}

	// a superclass with type arguments keeps its own gap
	class A6 extends (a + (b // c6
	) // d6
	)<T> {}

	// a block alone after the pair stays after the comment, in the order written
	class A7 extends (a + (b // c7
	) /* x7 */) {}

	// a lone comment stays ahead of type arguments too
	class A8 extends (a + (b // c8
	))<T> {}
</script>
