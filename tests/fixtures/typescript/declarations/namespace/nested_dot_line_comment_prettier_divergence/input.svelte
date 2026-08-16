<script lang="ts">
	// A line comment before the `.` — stays there, and the rest of the name continues
	// one level down
	namespace A // c1
		.B {
			export const a = 1;
		}

	// The gap on the other side of the dot
	namespace C. // c2
		D {
			export const a = 1;
		}

	// The `module` spelling and a deeper chain share the printer
	module E // c3
		.F.G {
			export const a = 1;
		}

	// A run stays in order, each comment on its own line
	namespace H // c4
		// c5
		.I {
			export const a = 1;
		}
</script>
