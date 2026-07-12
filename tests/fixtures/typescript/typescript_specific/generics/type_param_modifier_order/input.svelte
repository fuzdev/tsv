<script lang="ts">
	// `const` must precede any variance modifier; these valid orderings parse in both.
	interface Invariant<in out T> {}

	function withConst<const T>() {}
</script>
