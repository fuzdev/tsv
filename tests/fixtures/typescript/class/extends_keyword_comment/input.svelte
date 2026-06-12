<script lang="ts">
	// Comment between extends keyword and base class
	class A extends /* c */ B {}

	// Comment between name and extends keyword (correct)
	class C /* c */ extends D {}

	// Multiple comments around extends
	class E /* c1 */ extends /* c2 */ F {}

	// Class expression: comment between extends and base
	const G = class extends /* c */ H {};

	// Interface: comment between extends and first item
	interface I extends /* c */ J {}

	// Comment between implements keyword and first item
	class K implements /* c */ L {}

	// Multiple heritage with keyword comments
	class M /* c1 */ extends /* c2 */ N implements /* c3 */ O {}
</script>
