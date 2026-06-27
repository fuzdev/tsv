<script lang="ts">
	// TSTypeReference return + complex call body in chain: now expandable (3.9), sig hugged
	const a = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.method((x: string): Promise<any> =>
			anotherCallAAAAAAAAA({ key: valueAAAAAAAAAA })
		)
	};

	// Keyword return + complex call body in chain: IS expandable, sig hugged
	const b = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.method((x: string): void =>
			anotherCallAAAAAAAAAAAAAAAAA({ key: valueAAAAAAAAAAAA })
		)
	};
</script>
