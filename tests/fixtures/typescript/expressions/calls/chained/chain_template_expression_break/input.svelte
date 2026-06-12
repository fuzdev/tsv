<script lang="ts">
	// Chain inside template expression — Prettier breaks the chain when
	// the expression naturally contains newlines (multiline template callback).
	const feed = `header
  ${items
		.map(
			(item) => `
  <entry>
    <id>${item.id}</id>
    <title>${item.title}</title>
  </entry>`,
		)
		.join('\n')}
footer`;
</script>
