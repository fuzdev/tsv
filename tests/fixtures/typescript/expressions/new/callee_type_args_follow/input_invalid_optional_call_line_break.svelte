<script lang="ts">
	// Past a line break the list binds to the callee, which puts the `?.` in the callee of
	// `new` instead: an error either way.
	new f<T>
		?.(x);
</script>
