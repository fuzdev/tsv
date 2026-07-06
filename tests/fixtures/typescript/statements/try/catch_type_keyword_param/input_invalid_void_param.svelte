<script lang="ts">
	// `void` is a reserved operator, not a contextual keyword, so it is not a
	// valid catch binding name (unlike `any`/`string`/`object` above)
	try {
	} catch (void) {}
</script>
