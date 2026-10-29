<script lang="ts">
	// A multi-line block between the callee and its `(` leads the sole argument, and its
	// lines break the argument list open
	aaa.bbb/* c1
d1 */(() => ccc());
	[]/* c2
d2 */(() => ddd())();
	hhh/* c4
d4 */(() => iii());

	// A single-line block leads the argument inline
	eee.fff/* c3 */(() => ggg());
</script>
