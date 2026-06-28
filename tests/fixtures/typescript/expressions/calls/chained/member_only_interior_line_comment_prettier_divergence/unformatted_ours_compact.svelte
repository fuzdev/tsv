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
</script>
