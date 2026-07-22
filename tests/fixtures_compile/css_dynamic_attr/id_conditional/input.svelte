<script>
	let c = $state(true);
</script>

<p id={c ? 'a' : 'b'}>x</p>

<style>
	#a {
		color: red;
	}
</style>
