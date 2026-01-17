<script lang="ts">
	// Type expands, but } = init stays together (129 chars unformatted)
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaa: OmitStrict<Bbbbbbbbbbbbbb<typeof Ccccccccccc>, 'ddddddd'> & {eeeeeeee: Fffffff} = gggggg();
</script>
