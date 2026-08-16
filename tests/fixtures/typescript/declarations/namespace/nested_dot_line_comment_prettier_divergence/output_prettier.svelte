<script lang="ts">
	// A line comment before the `.` — stays there, and the rest of the name continues
	// one level down
	namespace A.B { // c1
		export const a = 1;
	}

	// The gap on the other side of the dot
	namespace C.D { // c2
		export const a = 1;
	}

	// The `module` spelling and a deeper chain share the printer
	module E.F.G { // c3
		export const a = 1;
	}

	// A run stays in order, each comment on its own line
	namespace H.// c5 // c4
	I {
		export const a = 1;
	}
</script>
