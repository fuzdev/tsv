<script lang="ts">
	// a union member inside a trailing-object intersection keeps its parens — `&` binds
	// tighter than `|`, so without them the type would mean something else
	type A1 = ((b | c) & { x: X }) | c;
	type A2 = (a & (b | c) & { x: X }) | c;

	// a function or constructor member keeps its parens — without them the type is invalid
	type A3 = (a & (() => void) & { x: X }) | c;
	type A4 = (a & (new () => void) & { x: X }) | c;

	// a conditional member keeps its parens
	type A5 = (a & (b extends c ? d : e) & { x: X }) | c;

	// the same shell as an optional tuple element
	type A6 = [(a & (b | c) & { x: X })?];
</script>
