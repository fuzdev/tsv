<script lang="ts">
	// A nested cast target reached through grouping parens keeps its assertion
	// wrapper and drops the parens, exactly as the unparenthesized form does:
	// acorn reads the parens as grouping, so they carry no meaning for
	// toAssignable here. `unformatted_paren` is the variant that spells them.
	({ a: b as T } = obj);
	({ a: b satisfies U } = obj);
	({ a: <V>b } = obj);
	({ a: b! } = obj);
	({ a: b.c as T } = obj);
	[c as T] = arr;
</script>
