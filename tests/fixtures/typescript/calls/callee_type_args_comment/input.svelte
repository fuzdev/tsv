<script lang="ts">
	// Block comment between callee and type args (empty call)
	fn1/* c1 */ <string>();

	// Block comment between type args and parens (empty call)
	fn2<string> /* c2 */();

	// Block comment between callee and type args (with args)
	fn3/* c3 */ <string>(1);

	// Block comment between type args and parens (with args)
	fn4<string>(/* c4 */ 1);

	// new expression: between callee and type args
	new Foo/* c5 */ <string>();

	// new expression: between type args and parens
	new Foo<string> /* c6 */();

	// Method call: between callee and type args
	obj.fn/* c7 */ <string>();

	// Method call: between type args and parens
	obj.fn<string> /* c8 */();
</script>
