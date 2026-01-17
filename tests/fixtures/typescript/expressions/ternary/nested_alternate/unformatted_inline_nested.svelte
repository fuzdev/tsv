<script>
	const value = $derived(
		conditionVariable === 'some_value'
			? firstResult?.propertyName === expectedValue
			: conditionVariable === 'other_value' ? secondResult === otherExpected : defaultFallbackValue,
	);
</script>
