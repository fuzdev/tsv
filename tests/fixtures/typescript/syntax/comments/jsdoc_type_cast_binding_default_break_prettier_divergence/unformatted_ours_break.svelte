<script>
	// A cast in a binding default keeps its comment glued to the `(` and expands inside the
	// preserved parens — the layout every wide cast takes.
	const { a1 = /** @type {A} */
	(b1) } = obj;

	const { a2 = /** @type {A} */
	(someObject.somePropertyName.someOtherProperty.andAnotherOne.more) } = obj;

	const [a3 = /** @type {A} */
	(someObject.somePropertyName.someOtherProperty.andAnotherOne.mor), a4 = /** @type {A} */
	(someObject.somePropertyName.someOtherProperty.andAnotherOne.mor)] = arr;

	function fn1(a5 = /** @type {A} */
	(someObject.somePropertyName.someOtherProperty.andAnother.m)) {}
</script>
