<script lang="ts">
	type A = [string, number?];
	type B = [string, number?, boolean?];
	type C = [first: string, second?: number];
	type D = [string, ...number[], boolean?];

	let a: [string, number?] = ['a'];
	let b: [string, number?] = ['b', 1];
</script>
