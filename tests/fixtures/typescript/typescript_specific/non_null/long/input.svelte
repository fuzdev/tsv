<script lang="ts">
	// Short non-null (contrast case)
	a!.b;

	// Long member chain with non-null
	veryLongObjectName000000!.veryLongPropertyName000000!.anotherVeryLongPropertyName000000!
		.yetAnotherProperty;

	// Long call chain with non-null
	veryLongFunctionName000000()!
		.veryLongMethodName000000()!
		.anotherVeryLongMethodName000000()!
		.finalMethod();

	// Long expression in parens with non-null
	(veryLongObjectName00000.veryLongPropertyName00000 ||
		alternativeVeryLongPropertyName00000)!.method();

	// Long await with non-null
	(await veryLongAsyncFunctionName(veryLongArgumentOne0000000, veryLongArgumentTwo0000000))!
		.property;

	// Long ternary with non-null
	(veryLongConditionExpression000000 ? veryLongTrueValue000000 : veryLongFalseValue000000)!
		.property;

	// Long type assertion with non-null
	(veryLongExpression00000000 as VeryLongTypeName0000000 | AnotherVeryLongTypeName0000000)!
		.property;

	// Assignment with long non-null chain
	const result =
		veryLongObjectName000000!.veryLongPropertyName000000!.anotherVeryLongPropertyName000000!;

	// Function call with long non-null args
	veryLongFunctionName(
		veryLongArgumentOne!.property,
		veryLongArgumentTwo!.property,
		veryLongArgumentThree!.property
	);

	// Nested long non-null in object
	const obj = {
		prop: veryLongObjectName0000000!.veryLongPropertyName0000000!.anotherVeryLongPropertyName000000!
	};
</script>
