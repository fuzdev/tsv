<script lang="ts">
	// An optional chain cannot follow an argument-less `new`: no argument list is inferred
	// ahead of the `?.`, so there is no `new f<T>()` for the call to chain from.
	new f<T>?.(x);
</script>
