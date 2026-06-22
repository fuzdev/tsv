<script lang="ts">
	// Chain with type args in array element - inline at exactly 100
	const a1 = [
		b.fn1<{ c: string }>(`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`).fn2(() => [{ c: '0' }])
	];

	// Chain with type args in array element - chain breaks at 101
	const a2 = [
		b
			.fn1<{ c: string }>(`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`)
			.fn2(() => [{ c: '0' }])
	];

	// Same with simple type arg - chain breaks at 101
	const a3 = [
		b
			.fn1<string>(`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`)
			.fn2(() => [{ c: '0' }])
	];
</script>
