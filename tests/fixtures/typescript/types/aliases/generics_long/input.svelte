<script lang="ts">
	// Single long type parameter breaks to multiple lines (PR #18043)
	export type OuterType2<
		LongerLongerLongerLongerInnerType = LongerLongerLongerLongerLongerLongerLongerLongerOtherType,
	> = {a: 1};

	// Multiple type parameters break to individual lines
	type MultiParam<
		A = VeryLongDefaultTypeName,
		B = AnotherVeryLongTypeName,
		C = YetAnotherVeryLongTypeName,
	> = A;
</script>
