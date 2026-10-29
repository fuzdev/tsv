<script lang="ts">
	// Two comments before `.baz`: the first trails `.bar`, the second on its own
	// line; the chain breaks at every member.
	const a=foo.bar// c1
// c2
.baz;

// A single own-line comment before a member.
	const b=foo.bar
// c1
.baz;

// A lookup with a comment in its brackets takes the same path: the gap comment after it
// keeps its own line.
	const c=arr[ // c3
i]
// c4
.baz;

// A block in the brackets, the same.
	const d=arr[/* c5 */ i]
// c6
.baz;
</script>
