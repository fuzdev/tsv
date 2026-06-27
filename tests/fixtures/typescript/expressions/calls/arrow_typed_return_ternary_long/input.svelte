<script lang="ts">
	// TSTypeReference return + ternary: now expandable (3.9), sig hugged
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

	// Keyword return (void) + ternary: IS expandable, uses hugged pattern
	const c = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAA: obj.fn((x: string): void =>
			cond ? aAAAAAAAAAAAAAAAAAAAAAAAAAAAA : bAAAAAAAAAAAAAAAAAAAAAAAA
		)
	};
</script>
