<script lang="ts">
	// Short - stays inline
	fn((x: number) => x);

	// Long expression arrow - outer wraps, params inline
	fn(
		(veryLongParamName1: VeryLongTypeName1, veryLongParamName2: VeryLongTypeName2) =>
			veryLongParamName1,
	);

	// Long expression arrow - params wrap individually
	fn(
		(
			veryLongParamName1: VeryLongTypeName1,
			veryLongParamName2: VeryLongTypeName2,
			veryLongParamName3: VeryLongTypeName3,
		) => veryLongParamName1,
	);

	// Long block arrow - stays hugged, params inline
	fn((veryLongParamName1: VeryLongTypeName1, veryLongParamName2: VeryLongTypeName2) => {
		return veryLongParamName1;
	});

	// Long block arrow - params wrap, body hugged
	fn(
		(
			veryLongParamName1: VeryLongTypeName1,
			veryLongParamName2: VeryLongTypeName2,
			veryLongParamName3: VeryLongTypeName3,
		) => {
			return veryLongParamName1;
		},
	);

	// Long return type - wraps params and union type
	fn(
		(
			x: number,
		):
			| VeryLongReturnTypeName1
			| VeryLongReturnTypeName2
			| VeryLongReturnTypeName3
			| VeryLongReturnTypeName4 => x,
	);

	// Long generic callback - type params inline, value params wrap
	fn(
		<VeryLongTypeParam1, VeryLongTypeParam2, VeryLongTypeParam3>(
			x: VeryLongTypeParam1,
			y: VeryLongTypeParam2,
		) => x,
	);

	// Long generic callback - type params wrap individually
	fn(
		<
			VeryLongTypeParam1,
			VeryLongTypeParam2,
			VeryLongTypeParam3,
			VeryLongTypeParam4,
			VeryLongTypeParam5,
		>(
			x: VeryLongTypeParam1,
		) => x,
	);

	// Long async callback - stays hugged, params inline
	fn(async (veryLongParamName1: VeryLongTypeName1, veryLongParamName2: VeryLongTypeName2) => {
		await fn(veryLongParamName1);
	});

	// Long async callback - params wrap, body hugged
	fn(
		async (
			veryLongParamName1: VeryLongTypeName1,
			veryLongParamName2: VeryLongTypeName2,
			veryLongParamName3: VeryLongTypeName3,
		) => {
			await fn(veryLongParamName1);
		},
	);
</script>
