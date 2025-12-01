<script lang="ts">
	// Short - stays inline
	[a, b, c] = arr;
	({a, b, c} = obj);

	// Long array - wraps
	[
		veryLongVariableName1,
		veryLongVariableName2,
		veryLongVariableName3,
		veryLongVariableName4,
	] = longArray;

	// Long object - wraps
	({
		propertyName1: renamed1,
		propertyName2: renamed2,
		propertyName3: renamed3,
		...restOfTheProperties
	} = longObject);

	// Long with computed keys
	({
		[veryLongKeyExpression1]: value1,
		[veryLongKeyExpression2]: value2,
		[veryLongKeyExpression3]: value3,
	} = longObject);

	// Long declaration (needs parser work)
	const {
		firstProperty,
		secondProperty,
		thirdProperty,
		fourthProperty,
	} = veryLongObjectName;

	// Long nested
	const {
		outer: {
			inner: {
				deep: value,
			},
		},
	} = veryDeepObject;

	// Long rest-only
	const {
		...veryLongRestVariableName
	} = objectWithManyProperties;
</script>
