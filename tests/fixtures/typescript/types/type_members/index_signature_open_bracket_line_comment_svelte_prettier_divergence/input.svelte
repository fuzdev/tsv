<script lang="ts">
	// A line comment the author wrote on the `[` line stays on that line; prettier
	// relocates it to its own line as the key's leading comment.
	interface A {
		[ // b
			key: string
		]: number;
	}
</script>
