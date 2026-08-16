<script lang="ts">
	// A line comment between a destructuring pattern's closing bracket and its
	// `: Type` annotation keeps the comment after the bracket; the annotation
	// drops to a continuation line indented one level. An object pattern's
	// braces expand under the forced break, an array pattern's brackets do not.
	const {
		a
	} // c1
		: T = x;

	const [b] // c2
		: U = y;
</script>
