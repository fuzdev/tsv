<script lang="ts">
	// `const in out` is the canonical modifier order, each modifier once, and the only
	// form either formatter emits — the reversed and repeated spellings normalize to it
	// (see the variants).
	class Ordered<const in T> {}
	class OrderedOut<const out T> {}

	interface Invariant<in out T> {}

	function withConst<const T>() {}
</script>
