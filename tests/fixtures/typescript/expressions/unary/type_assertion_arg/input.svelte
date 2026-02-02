<script lang="ts">
	// Type assertions need parens in unary argument context
	let a = !(<T>foo());
	let b = !(bar() as T);
	let c = !(baz() satisfies T);

	// Other unary operators
	let d = typeof (<T>foo());
	let e = void (<T>foo());
</script>
