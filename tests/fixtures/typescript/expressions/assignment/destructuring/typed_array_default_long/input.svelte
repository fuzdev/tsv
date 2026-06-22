<script lang="ts">
	// Array destructuring + tuple type + default: 100 chars - stays inline
	function fn1([a, b]: [string, string, string, string, string, string, stringABCDEFGHIJKL] = []) {}

	// Array destructuring + tuple type + default: 101 chars - tuple type expands, pattern stays inline
	function fn2([a, b]: [
		string,
		string,
		string,
		string,
		string,
		string,
		stringABCDEFGHIJKLM
	] = []) {}

	// Without default: 101 chars - tuple type expands, pattern stays inline (same behavior)
	function fn3([a, b]: [
		string,
		string,
		string,
		string,
		string,
		string,
		string,
		string,
		stringABCDE
	]) {}

	// Arrow function variant with default
	const fn4 = ([a, b]: [
		string,
		string,
		string,
		string,
		string,
		string,
		stringABCDEFGHIJKLM
	] = []) => {};
</script>
