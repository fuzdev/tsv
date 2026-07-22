<script lang="ts">
	function first<T>(list: T[]): T | undefined {
		return list[0];
	}

	const pick = <T,>(list: T[], index: number): T => list[index];

	let a = first<number>([1, 2]);
	let b = pick([3, 4], 1);
</script>

<p>{a}{b}</p>
