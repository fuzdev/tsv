<script lang="ts">
	// a directive trailing a member's line is inert in both directions —
	// type-literal and interface member lists
	type M = {
		a: 1; // prettier-ignore
		b: 2;
	};
	interface B {
		a: 1; // prettier-ignore
		b: 2;
	}
</script>
