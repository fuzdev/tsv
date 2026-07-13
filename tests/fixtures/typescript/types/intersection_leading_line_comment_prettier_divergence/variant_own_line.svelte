<script lang="ts">
	// case C: first intersection member with leading line comment
	type C =
		// leading
		a & b;

	// case M3: nested union inside intersection
	type M3 =
		// leading
		(a | b) & c;
</script>
