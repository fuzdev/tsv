<script lang="ts">
	// A non-last argument of a chained call with a block comment after the comma and
	// a line comment after it: the block stays on the comma line, the line comment
	// trails the comma.
	foo.bar(a,/* c1 */// c2
	b).baz();
</script>
