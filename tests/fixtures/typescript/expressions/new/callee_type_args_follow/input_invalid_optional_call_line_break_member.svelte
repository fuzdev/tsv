<script lang="ts">
	// The member after the break is part of the callee, so the `?.` after it is too.
	new f<T>
		[x]?.(y);
</script>
