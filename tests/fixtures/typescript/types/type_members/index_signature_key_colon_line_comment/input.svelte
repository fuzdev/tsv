<script lang="ts">
	// A line comment between the key and `:` forces the bracket to break so the
	// `//` can't swallow the `: type`; the comment stays after the key, the value
	// type drops to its own line.
	interface A {
		[
			key // c
			: string
		]: number;
	}
</script>
