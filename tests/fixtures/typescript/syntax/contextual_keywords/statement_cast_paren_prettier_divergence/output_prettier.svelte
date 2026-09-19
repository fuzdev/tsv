<script lang="ts">
	// a word tsc reads as a declaration head keeps its pair ahead of a cast
	namespace as T;
	namespace satisfies T;

	// a modifier tsc reads ahead of a declaration keeps its pair too
	accessor as T;
	readonly satisfies T;

	// a `for` init opens a `using` declaration the same way
	for (using as T; ;) {}
</script>
