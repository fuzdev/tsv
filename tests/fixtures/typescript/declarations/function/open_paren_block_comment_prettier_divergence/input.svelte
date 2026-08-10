<script lang="ts">
	// A block comment the author left on the `(` line leads the first parameter, on the
	// parameter's line when the list fits
	function fn1(/* c */ a) {}

	// and above it when the list breaks
	function fn2(
		/* c */
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong: string
	) {}

	// A function type keeps it in the same place
	type A = (/* c */ a: string) => void;
</script>
