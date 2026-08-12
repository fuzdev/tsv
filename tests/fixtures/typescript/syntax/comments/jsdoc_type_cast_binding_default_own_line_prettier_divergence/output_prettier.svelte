<script>
	// A cast comment the author gave a line of its own hangs the value: the `=` ends the
	// line, and the comment and its `(` sit indented under it — the layout every other
	// value gap gives an own-line cast.
	const {
		/** @type {A} */
		objDefault = (someObject.somePropertyName.someOtherProperty.andAnotherOne.more)
	} = obj;

	const [
		/** @type {A} */
		arrDefault = (someObject.somePropertyName.someOtherProperty.andAnotherOne.mor)
	] = arr;

	function fn1(
		/** @type {A} */
		paramDefault = (someObject.somePropertyName.someOtherProperty.andAnother.m)
	) {}
</script>
