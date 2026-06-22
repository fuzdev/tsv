<script lang="ts">
	type A = { kind: 'a'; value: number };
	type B = { kind: 'b'; name: string };
	type C = { kind: 'c'; flag: boolean };
	type Union = A | B | C;

	function fn(x: Union) {
		if (x.kind === 'a') {
			return x.value;
		}
	}
</script>
