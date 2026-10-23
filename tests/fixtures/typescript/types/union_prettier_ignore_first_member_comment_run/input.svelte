<script lang="ts">
	// a comment between the directive and the first member keeps the run unbroken
	type A =
		// prettier-ignore
		// x
		{  a:1 } | B;

	// the same when the comment's text ends in a byte the walk steps over
	type C =
		// prettier-ignore
		// x |
		{  a:1 } | B;

	type D =
		// prettier-ignore
		// x (
		{  a:1 } | B;

	type F =
		// prettier-ignore
		// x &
		{  a:1 } | B;

	type E =
		// prettier-ignore
		/* x */
		{  a:1 } | B;
</script>
