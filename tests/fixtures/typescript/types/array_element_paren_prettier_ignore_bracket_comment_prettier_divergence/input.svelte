<script lang="ts">
	// the bracket suffix is outside the freeze, so a comment in it stays in the brackets
	type A = (
		// prettier-ignore
		(a:  string) => void
	)[/* c */];
</script>
