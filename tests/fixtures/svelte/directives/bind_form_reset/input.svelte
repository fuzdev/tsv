<script>
	let value = $state('');
	let checked = $state(true);
	let selected = $state('b');
</script>

<form>
	<input bind:value defaultValue="default text" />
	<input type="checkbox" bind:checked defaultChecked={true} />
	<select bind:value={selected}>
		<option value="a">A</option>
		<option value="b" selected>B</option>
		<option value="c">C</option>
	</select>
	<button type="reset">Reset</button>
</form>
