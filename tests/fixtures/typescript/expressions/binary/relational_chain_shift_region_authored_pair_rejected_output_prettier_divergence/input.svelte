<script lang="ts">
	// The pair stays where the stripped line parses as a type-argument list that nothing
	// may follow: a regex past the `>>`, and a `>>` inside a bracket or a pair the region
	// holds, which tsc's list parse reads through
	x1 = (f < A) < C >> /r/.x;
	x2 = (f < A) < [C >> `t`];
	x3 = (f < A) < (C >> `t`) + 1;
	fn((f < A) < C >> /r/.x);

	// A `<<` opens the region too: stripped, tsc reads the list and rejects the line
	x4 = (f << A) < C >> `t`;
</script>
