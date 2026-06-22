<script lang="ts">
	{
		const from = '';
		const to = '';
	}

	{
		const [from, to] = arr;
	}

	{
		for (const [from, to] of pairs) {
		}
	}

	{
		const { from, to } = obj;
	}

	function fn1(from: number, to: number) {}

	const fn2 = ([from, to]: [number, number]) => from + to;

	// from as expression statement
	from.shift();
	from = 'a';
</script>
