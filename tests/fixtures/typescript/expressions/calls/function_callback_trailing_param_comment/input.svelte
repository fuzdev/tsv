<script lang="ts">
	// A forced break in the signature refuses the hug, so the call expands
	fn(
		function (
			y // c1
		) {
			call(y);
		}
	);

	// A multiline block forces it too
	fn(
		function (
			y /* c2
	c2 */
		) {
			call(y);
		}
	);

	// An own-line block forces it too
	fn(
		function (
			y
			/* c3 */
		) {
			call(y);
		}
	);

	// A named function expression takes the same path
	fn(
		function fn1(
			y // c4
		) {
			call(y);
		}
	);

	// An async function expression too
	fn(
		async function (
			y // c5
		) {
			call(y);
		}
	);

	// A generator function expression too
	fn(
		function* (
			y // c6
		) {
			call(y);
		}
	);

	// A member chain's argument too
	a.b().c(
		function (
			y // c7
		) {
			call(y);
		}
	);

	// The arrow twin, for contrast in the same file
	fn(
		(
			y // c8
		) => {
			call(y);
		}
	);

	// Control: a `new` expression hugs a function expression, where its arrow twin expands
	new Comp(function (
		y // c9
	) {
		call(y);
	});

	// Control: a single-line block forces nothing, so the hug stands
	fn(function (y /* c10 */) {
		call(y);
	});

	// Control: a comment before the last parameter does not reach this question
	fn(function (y /* c11 */, w) {
		call(y);
	});

	// Control: no comment
	fn(function (y) {
		call(y);
	});
</script>
