<script lang="ts">
	// A leading run whose multi-line block the author broke after: the break after its
	// `*/` survives, so the right operand leads the continuation line at the run's indent.

	// `+`
	const a =
		x + (
		/* c1 */ /* c2
		d2 */
		y);

	// `&&`
	const b =
		x && (
		/* c3 */ /* c4
		d4 */
		y);

	// `||`, the multi-line block first in the run
	const c =
		x || (
		/* c5
		d5 */ /* c6 */
		y);

	// `??`
	const d =
		x ?? (
		/* c7 */ /* c8
		d8 */
		y);

	// a blank line after the run survives
	const e =
		x + (
		/* c9 */ /* c10
		d10 */

		y);

	// an expression statement, not an initializer
	x + (
		/* c11 */ /* c12
		d12 */
		y);

	// control: the run glued to the operand keeps it on the closing line
	const f =
		x + (
		/* c13 */ /* c14
		d14 */ y);

	// control: a single-line run the author broke after collapses onto one line
	const g = x + (
		/* c15 */ /* c16 */ y);
</script>
