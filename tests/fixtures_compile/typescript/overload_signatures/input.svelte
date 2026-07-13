<script lang="ts">
	function f(x: number): number;
	function f(x: string): string;
	function f(x: any) {
		return x;
	}

	let a = f(1);
</script>

<p>{a}</p>
