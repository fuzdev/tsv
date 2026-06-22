<script lang="ts">
	// Single-arg: void return + call body - sig hugged, body breaks after =>
	const a = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: fn((x: string): void => call(undefined))
	};

	// Single-arg: string return + call body - same behavior
	const b = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: fn((x: string): string =>
			call(undefined)
		)
	};

	// Multi-arg: void return + call body - head args inline, arrow sig hugged
	const c = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: fn({ key: 'value' }, (x: string): void =>
			call(undefined)
		)
	};

	// Contrast: TSTypeReference return - NOT expandable, breaks at open paren
	const d = {
		propAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: fn(
			(x: string): Promise<any> => call(undefined)
		)
	};
</script>
