<script lang="ts">
	// `const in out` is the canonical modifier order, each modifier once, and the only
	// form either formatter emits — the reversed and repeated spellings normalize to it
	// (see the variants).
	class Ordered<const const in T> {}
	class OrderedOut<const out out T> {}

	interface Invariant<in in out out T> {}

	function withConst<const const T>() {}
</script>
