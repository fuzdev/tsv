<script lang="ts">
	// An own-line block comment after a parameter's `=` default collapses to the
	// inline form under tsv (the comment stays after `=`, on the value). Prettier
	// also keeps it inline but moves it before `=`. See unformatted_ours_*.
	function f(
		/* c */
		a = b
	) {}

	// Object destructuring binding default: same rule.
	const {
		/* c */
		x = y
	} = o;

	// Array destructuring binding default: same rule.
	const [
		/* c */
		z = w
	] = arr;
</script>
