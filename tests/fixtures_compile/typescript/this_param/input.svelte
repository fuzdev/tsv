<script lang="ts">
	function f(this: any, x: number) {
		return x;
	}

	let a = f(1);
</script>

<p>{a}</p>
