<script lang="ts">
	a ? b : c = 1;
	() => b = 1;
	async () => b = 1;
	x = a ? b : c = 1;
	[a ? b : c = 1] = x;
	function* fn() {
		yield a = 1;
	}
</script>
