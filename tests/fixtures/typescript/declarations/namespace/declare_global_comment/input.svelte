<script lang="ts">
	// comment between `declare` and `global`
	declare /* a */ global {}

	// comment between `global` and the body brace
	declare global /* b */ {}

	// comment inside the body
	declare global {
		/* c */
	}
</script>
