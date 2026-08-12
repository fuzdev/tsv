<script>
	// A cast whose comment is glued to its `(` keeps the value on the comment's line and
	// expands inside the preserved parens — the layout every wide cast takes.
	const a1 = /** @type {A} */ (b1);

	const a2 =
		/** @type {A} */
		(someObject.somePropertyName.someOtherProperty.andAnotherOne.andYetMore.final);

	obj.prop =
		/** @type {A} */
		(someObject.somePropertyName.someOtherProperty.andAnotherOne.andYetMore.fina);

	class K {
		field =
			/** @type {A} */
			(someObject.somePropertyName.someOtherProperty.andAnotherOne.andYetMor.fin);
	}

	const fn1 = () =>
		/** @type {A} */
		(someObject.somePropertyName.someOtherProperty.andAnotherOne.andYetMore.finalOne);

	// A value gap inside the cast (an object property, an arrow body) marks itself while the
	// inner is built, so the cast's own answer has to be taken before that recursion.
	const a3 =
		/** @type {A} */
		({ someKeyName: someValueName, anotherKeyName: anotherValue, thirdKey: third });

	const a4 =
		/** @type {A} */
		((someParameter) => someObject.someProperty.someOther.andAnother.andYetMore.fin);
</script>
