<script lang="ts">
	// A no-args `new` synthesizes its `()`, but the callee's stripped grouping parens
	// still hold a gap: the comment there is kept, before the synthesized list.
	const a = new Foo /* c1 */();
	const b = new Foo.Bar /* c2 */();
	const c = new Foo`x` /* c3 */();
	const d = new Foo<T> /* c4 */();
</script>
