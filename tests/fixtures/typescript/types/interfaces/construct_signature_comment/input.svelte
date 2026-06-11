<script lang="ts">
	interface A {
		// a comment between `new` and the params keeps the construct signature
		new (/* c1 */ a: number): A;
		new /* c2 */ <T>(b: T): A;

		// `new` as a member name for contrast
		new: string;
	}
</script>
