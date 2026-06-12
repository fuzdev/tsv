<script>
	// ArrayExpression
	let arr1 = [/* c */ 1];
	let arr2 = [1 /* c */];
	let arr3 = [/* a */ /* b */ 1];

	// CallExpression
	foo(/* c */ a);

	// MemberExpression
	a /* c */.b;

	// ComputedMemberExpression
	obj[/* c */ key];

	// ConditionalExpression
	let cond = a /* c */ ? b : c;

	// BinaryExpression
	let bin = a /* c */ + b;

	// TemplateLiteral
	let tmpl = `${/* c */ a}`;

	// ArrowFunctionExpression
	let arrowParam = (/* c */ a) => a;
	let arrowParams = (a, /* c */ b) => a + b;
	let arrowBody = () => /* c */ {};

	// Nested
	let nested = [/* outer */ [/* inner */ 1]];
</script>
