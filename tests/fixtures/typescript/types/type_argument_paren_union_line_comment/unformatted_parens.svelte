<script lang="ts">
	// line comment in a parenthesized union type argument
	type A1 = Foo<(a | b // c
	)>;
</script>
