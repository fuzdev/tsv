<script lang="ts">
	function f(x): x is ((/* b */ // c
		A)) {}

	function g(x): x is ((// c
		B /* t */)) {}
</script>