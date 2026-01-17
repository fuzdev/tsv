<script lang="ts">
	// 100 chars exactly - both stay inline
	function f(): `${LongTypeNameAaaaaaaaaaaaaaaaaaaaaaa extends OtherType ? TrueType : FalseType}` {}

	// 101 chars - Prettier inline, we break at ?/:
	function g(): `${LongTypeNameAaaaaaaaaaaaaaaaaaaaaaaa extends OtherType
		? TrueType
		: FalseType}` {}

	// Type alias with conditional - same pattern
	type T =
		`prefix_${LongTypeNameAaaaaaaaaaaaaaaaaaaaaa extends OtherType ? TrueType : FalseType}_suffix`;
</script>
