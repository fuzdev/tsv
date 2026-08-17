<script lang="ts">
	// the `...`→binding gap
	const [a, .../* c */b] = x;
	const {c: d, .../* c */e} = y;
	function f(.../* c */args: string[]) {}
	type F = (.../* c */args: string[]) => void;

	// the binding→`:` gap of the same rest element
	function g(...args/* c */: string[]) {}
	type G = (...args/* c */: string[]) => void;

	// a destructuring rest binding keeps the gap comment outside the brackets
	function h(...[a]/* c */: string[]) {}
</script>
