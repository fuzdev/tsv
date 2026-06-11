<script lang="ts">
	// a comment between `new` and the params stays where the user put it
	let a: new/* c1 */() => A;
	let b: new/* c2 */(x: number) => A;
	let c: abstract new/* c3 */() => A;
</script>
