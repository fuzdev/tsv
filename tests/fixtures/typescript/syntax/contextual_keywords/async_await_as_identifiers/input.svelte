<script lang="ts">
	// async is fully contextual - works everywhere
	{
		const async = 'a';
	}
	{
		let async = 'b';
	}
	{
		const { async } = obj;
	}
	{
		const [async] = arr;
	}
	function fn1(async: string) {
		return async;
	}
	const fn2 = (async: string) => async;
	{
		const async = 'a';
		const obj2 = { async };
	}
	const obj3 = { async: 'value' };
	const obj4 = {
		async() {
			return 'a';
		}
	};
	{
		let async;
		async = 'updated';
	}

	// await is more restricted - only works in property positions
	const obj5 = { await: 'value' };
	const obj6 = {
		await() {
			return 'a';
		}
	};
	{
		const { await: a } = obj;
	}

	// Inside async functions, await is the operator
	async function fn3() {
		const result = await promise;
	}
</script>
