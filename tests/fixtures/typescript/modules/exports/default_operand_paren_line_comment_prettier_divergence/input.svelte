<script>
	// A `//` written inside the exported value's own grouping parens stays inside them.
	// Deferring it past the `;` would land it on a line that may already hold one, and
	// two `//`s sharing an output line merge into a single comment.
	export default (
		x // c1
	); // c2
</script>
