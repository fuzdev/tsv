<script lang="ts">
	//
	// Blank Line Before First Method Only - Should Collapse
	//
	// When there's a blank line before the first method but NO blank lines
	// between subsequent methods, Prettier collapses everything to inline.
	// This is different from having blank lines BETWEEN methods (which preserve).

	// Basic case - short chain fits on one line
	const a = obj.method1().method2();

	// With property access before method
	const b = obj.prop.method1().method2();

	// Inside template literal
	const c = `${obj.method1().method2()}`;

	// Inside template with surrounding text
	const d = `prefix ${obj.method1().method2()} suffix`;

	// Longer chain that still fits
	const e = obj.method1().method2().method3();

	// With optional chaining
	const f = obj?.method1()?.method2();
</script>
