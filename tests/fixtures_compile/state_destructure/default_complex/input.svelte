<script>
	let obj = {};
	function f() {}
	let { a = f() } = $state(obj);
</script>

{a}
