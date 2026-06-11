<script lang="ts">
	interface A {
		// a comment between `new` and the params stays where the user put it
		new /* c1 */ (a: number): A;
		new /* c2 */ (): A;
		new /* c3(d) */ (b: string): A;
		new // c5
		(c: boolean): A;
	}

	type B = {
		new /* c4 */ (a: number): B;
	};
</script>
