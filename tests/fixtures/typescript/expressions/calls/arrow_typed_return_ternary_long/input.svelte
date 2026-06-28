<script lang="ts">
	// TSTypeReference return + ternary: sig hugged, body breaks after =>
	const a = {
		propAAAAAAAAAAAAAAAAAAAAAAA: obj.fn((x: string): Promise<any> =>
			cond ? aAAAAAAAAAAAAA : bAAAAAAAAAAAA
		)
	};

	// Non-chain: same behavior
	const b = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: fn((x: string): Promise<any> =>
			cond ? aAAAAAAAAAAAAA : bAAAAAAAAAAAA
		)
	};

	// Keyword return (void) + ternary: sig hugged, body breaks after =>
	const c = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.fn((x: string): void =>
			cond ? aAAAAAAAAAAAAAAAAAAAAAAAAAAAA : bAAAAAAAAAAAAAAAAAAAAAAAA
		)
	};
</script>
