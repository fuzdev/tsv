<script>
	/* a blank line between numbers is kept, and forces the array to expand */
	const a = [
		1, 2,

		3, 4
	];

	/* several blank lines collapse to one */
	const b = [
		1, 2,



		3, 4
	];

	/* a blank line inside a long fill - the runs before and after it still pack */
	const c = [
		1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
		27, 28, 29, 30,

		31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54,
		55, 56, 57, 58, 59, 60
	];

	/* a blank line before the first or after the last number is not between items - dropped */
	const d = [1, 2];
</script>
