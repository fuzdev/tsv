<script lang="ts">
	// TSTypeReference return + complex call body in chain: sig hugged, body breaks after =>
	const a = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.method((x: string): Promise<any> =>
			anotherCallAAAAAAAAA({ key: valueAAAAAAAAAA })
		)
	};

	// Keyword return + complex call body in chain: sig hugged, body breaks after =>
	const b = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.method((x: string): void =>
			anotherCallAAAAAAAAAAAAAAAAA({ key: valueAAAAAAAAAAAA })
		)
	};
</script>
