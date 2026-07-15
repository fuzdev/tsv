<script lang="ts">
	// A line comment before the `.` — stays there, and the rest of the name continues
	// one level down
	let a: ns.Type; // c1

	// The gap on the other side of the dot
	let b: ns.Type; // c2

	// Interface heritage shares the qualified-name printer
	interface Extends
		extends ns // c3
		.Base {}

	// A type alias puts a breaking right-hand side below the `=`, so the name's own
	// continuation nests inside that
	type A = ns.Type; // c4
</script>
