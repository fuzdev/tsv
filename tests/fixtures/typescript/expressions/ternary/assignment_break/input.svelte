<script>
	// Dynamic import based on condition (binary)
	const value =
		conditionVariable === 'some_long_value'
			? await import('./path/to/module/a')
			: await import('./path/to/module/b');

	// Function calls in branches (binary)
	const someResult =
		someLongCondition === 'value_that_is_longer'
			? await firstBranchFunc()
			: await secondBranchFunc();

	// Logical expression in condition
	const logicalResult =
		conditionA && conditionB
			? await import('./path/to/module/a')
			: await import('./path/to/module/b');
</script>
