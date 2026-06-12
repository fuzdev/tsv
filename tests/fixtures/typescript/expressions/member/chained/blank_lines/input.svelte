<script lang="ts">
	//
	// Member-Only Chains with Blank Lines - Should Collapse
	//
	// Blank lines in member-only chains (no calls) are always collapsed.
	// This differs from call chains where blank lines between calls are preserved.

	// Basic member chain - blank line collapses
	const a = obj.prop1.prop2.prop3;

	// Multiple blank lines - all collapse
	const b = obj.prop1.prop2.prop3.prop4;

	// With optional chaining
	const c = obj?.prop1?.prop2?.prop3;

	// Long chain that must break - blank lines still collapse
	const d =
		veryLongObjectName00000000.veryLongPropertyName1.veryLongPropertyName2.veryLongPropertyName3;

	// Non-null assertions in member chain (inline, no blank line variant)
	const e = obj!.prop1!.prop2!.prop3;

	// Mixed optional and non-null (inline, no blank line variant)
	const f = obj?.prop1!.prop2?.prop3;
</script>
