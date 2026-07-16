<script lang="ts">
	// Block comment before the `.` of a dotted namespace — stays there
	namespace A /* c1 */.B {
		export const a = 1;
	}

	// Block comment after the `.` — stays on that side
	namespace C./* c2 */ D {
		export const a = 1;
	}

	// Both gaps in a deeper chain
	namespace E./* c3 */ F /* c4 */.G {
		export const a = 1;
	}
</script>
