<script lang="ts">
	// `const in out` is the canonical modifier order, each modifier once, and the only
	// form either formatter emits — the reversed and repeated spellings normalize to it
	// (see the variants).
	class Ordered<in const T> {}
	class OrderedOut<out const T> {}

	interface Invariant<out in T> {}

	function withConst<const T>() {}
</script>
