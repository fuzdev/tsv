<script lang="ts">
	// Block comment before the `.` — stays there, hugging the dot
	type A = ns /* c1 */.Type;

	// Block comment after the `.` — stays on that side
	type B = ns./* c2 */ Type;

	// Both gaps in a deeper chain
	type C = a.b /* c3 */.c./* c4 */ Type;

	// Interface heritage shares the qualified-name printer
	interface Extends extends ns./* c5 */ Base {}

	// So does an import-equals module reference
	import Ref = ns./* c6 */ Sub;
</script>
