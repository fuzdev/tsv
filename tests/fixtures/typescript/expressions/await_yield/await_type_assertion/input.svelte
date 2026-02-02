<script lang="ts">
	// All type assertion forms need parens in await context
	let a = await (<T>foo());
	let b = await (bar() as T);
	let c = await (baz() satisfies T);

	// Type assertion with member access (chain base also needs parens)
	let d = (await (<T>foo())).prop;
</script>
