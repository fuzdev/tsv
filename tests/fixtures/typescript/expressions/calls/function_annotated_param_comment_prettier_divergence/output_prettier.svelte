<script lang="ts">
	// An annotated last parameter: tsv expands like every other forced break
	fn(function (
		y: T // c1
	) {
		call(y);
	});

	// The type's shape does not matter
	fn(function (
		y: string // c2
	) {
		call(y);
	});

	// A default value puts a node other than the identifier before the comment too
	fn(function (
		y = 1 // c3
	) {
		call(y);
	});

	// A destructured last parameter, likewise
	fn(function (
		{ y } // c4
	) {
		call(y);
	});

	// The break-forcing kind does not matter either
	fn(function (
		y: T /* c5
	c5 */
	) {
		call(y);
	});

	// A member chain's argument takes the same path
	a.b().c(function (
		y: T // c6
	) {
		call(y);
	});

	// Control: the arrow twin converges here — prettier expands it whatever the parameter
	fn(
		(
			y: T // c7
		) => {
			call(y);
		}
	);
</script>
