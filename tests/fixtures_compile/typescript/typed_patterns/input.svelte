<script lang="ts">
	function sum({ x, y = 2 }: { x: number; y?: number }, [head]: number[], ...rest: number[]) {
		return x + y + head + rest.length;
	}

	let total = sum({ x: 1 }, [3], 4, 5);
</script>

<p>{total}</p>
