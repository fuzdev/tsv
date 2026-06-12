<script lang="ts">
	//
	// Blank Line After Member Access - Should Collapse
	//
	// Blank lines after member access (before a call) are collapsed.
	// This is Rule 2: only blank lines after calls are preserved.

	// Blank line after .prop, before .method() - collapses
	const a = obj.prop.method();

	// Blank line after call result's member - collapses
	const b = obj.method().prop.method2();

	// Multiple members then call - collapses
	const c = obj.prop1.prop2.method();

	// Chain with multiple transitions - only blank after () preserved
	const d = obj.prop1.method1().prop2.method2();

	// Optional chaining variant
	const e = obj?.prop?.method();

	// Long chain that breaks - blank after member still collapses
	const f = veryLongObjectName.veryLongPropertyName.veryLongMethodName();
</script>
