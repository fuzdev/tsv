<script lang="ts">
	// A postfix update ends an `UpdateExpression`, which takes no type argument list, so a
	// `<`…`>` after one is a comparison chain on any line — under a prefix operator or an
	// angle-bracket assertion as on its own.
	a1 = !b++
<T>(z);
	a2 = -b--
<T>(z);
	a3 = typeof b++
<T>`t`;
	a4 = <U>b++
<T>(z);
	a5 = b++
<T>(z);

	// No call continues one either, so a `(` line after one is a new statement.
	c1 = <T>b++
(z);
</script>
