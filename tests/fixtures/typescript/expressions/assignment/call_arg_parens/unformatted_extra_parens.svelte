<script lang="ts">
	// Assignment expressions need parens in various contexts for clarity

	// Call arguments
	fn(((a = b)));
	fn(x, ((a = b)));
	fn(((a = b)), y);
	fn(((a = b)), ((c = d)));

	// Compound assignment in call args
	fn(((a += b)));
	fn(((a -= b)));
	fn(((a *= b)));

	// Logical assignment in call args
	fn(((a ||= b)));
	fn(((a &&= b)));
	fn(((a ??= b)));

	// Array elements
	const arr1 = [((a = b))];
	const arr2 = [x, ((a = b))];
	const arr3 = [((a = b)), ((c = d))];

	// New expression arguments
	new Fn(((a = b)));
	new Fn(x, ((a = b)));

	// Nested call
	outer(inner(((a = b))));

	// Method call
	obj.method(((a = b)));

	// Template literal expressions
	const t1 = `${((a = b))}`;
	const t2 = `prefix ${((a = b))} suffix`;
	const t3 = `${x} ${((a = b))} ${y}`;

	// Computed property keys
	const obj1 = {[((a = b))]: c};
	const obj2 = {x, [((a = b))]: c, y};
</script>
