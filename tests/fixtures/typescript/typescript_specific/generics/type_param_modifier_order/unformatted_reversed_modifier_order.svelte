<script lang="ts">
	// `const in out` is the canonical modifier order, and the only one either
	// formatter emits — the reversed spellings normalize to it (see the variant).
	class Ordered<in const T> {}
	class OrderedOut<out const T> {}

	interface Invariant<in out T> {}

	function withConst<const T>() {}
</script>
