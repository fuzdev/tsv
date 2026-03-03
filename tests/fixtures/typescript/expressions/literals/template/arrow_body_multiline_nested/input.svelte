<script lang="ts">
	const x = `
  ${items
		.map(
			(item) =>
				`
    <x>
      <y>${item.a}</y>
      <z>${item.b}</z>
    </x>`,
		)
		.join('')}
`;
</script>
