<script lang="ts">
	// Long value type - wraps union across lines (exceeds 100 chars)
	type LongValue = {
		[key: string]:
			| VeryLongTypeName
			| AnotherLongTypeName
			| ExtraLongTypeName
			| MoreTypesHere
			| EvenMoreTypes;
	};

	// Short value type - stays on one line (under 100 chars)
	type ShortValue = {[key: string]: string | number | boolean};

	// Long readonly - wraps when exceeds 100 chars
	type LongReadonly = {
		readonly [key: string]:
			| VeryLongTypeName
			| AnotherLongTypeName
			| ExtraLongTypeName
			| MoreTypesNow;
	};

	// Long mixed - properties with long index signature value
	type LongMixed = {
		name: string;
		[key: string]:
			| VeryLongTypeName
			| AnotherLongTypeName
			| ThirdLongTypeName
			| FourthLongType
			| FifthLongType;
	};

	// Long number index - wraps value type (exceeds 100 chars)
	type LongNumIndex = {
		[idx: number]:
			| VeryLongTypeName
			| AnotherLongTypeName
			| ExtraLongTypeName
			| MoreTypesNowHereNowX;
	};

	// Long key type - brackets wrap to separate lines (exceeds 100 chars)
	type LongKeyType = {
		[
			key: string0000000000000000000000000000000000000000000000000000000000000000000000000000000000000
		]: number;
	};

	// Long readonly key type - brackets wrap
	type LongReadonlyKey = {
		readonly [
			key: string0000000000000000000000000000000000000000000000000000000000000000000000000000000000000
		]: number;
	};

	// Long key with union key type - both key and union wrap
	type LongUnionKey = {
		[
			key:
				| string000000000000000000000000000000000000000000000000000000000000000000000000000000000
				| number
		]: unknown;
	};
</script>
