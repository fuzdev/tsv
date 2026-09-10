<script lang="ts">
	// prettier-ignore
	fn(  a  ) /* c1 */;

	// prettier-ignore
	const b  =  x /* c2 */;

	// prettier-ignore
	debugger /* c3 */;

	// prettier-ignore
	fn(  c  ) // c4
	;
	// an author blank INSIDE the slice is printed once, by the slice itself
	// prettier-ignore
	fn(  d  )

	// c5
	;
	fn1();

	// prettier-ignore
	fn(  e  )

	/* c6 */;
	fn2();
</script>
