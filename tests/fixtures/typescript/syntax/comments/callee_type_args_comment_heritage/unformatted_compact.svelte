<script lang="ts">
	// Class declaration extends: comment between super class and type args
	class A extends Base/* c1 */<T> {}

	// Class expression extends: comment between super class and type args
	const a = class extends Base/* c2 */<T> {};

	// Import type: comment between qualifier and type args
	type B = import('a').Foo/* c3 */<string>;

	// Type reference: comment between name and type args
	type C = Map/* c4 */<string, number>;

	// Type query: comment between expression and type args
	type D = typeof fn1/* c5 */<string>;

	// Heritage implements: comment between name and type args
	class E implements Foo/* c6 */<T> {}

	// Type annotation with wrapping: comment between name and type args
	let x: Promise/* c7 */<string> = y;
</script>
