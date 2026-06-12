<script lang="ts">
	// Long intersection - type literal stays inline, wrap at = instead
	const props: OmitStrict<ComponentProps<typeof Contextmenu>, 'entries'> & {children: Snippet} =
		$props();

	// Multiple properties still expands
	const a: OmitStrict<ComponentProps<typeof Contextmenu>, 'entries'> & {
		a: A;
		b: B;
	} = $props();
</script>
