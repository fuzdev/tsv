<script lang="ts">
	// a directive trailing the opening brace is inert — type-literal and interface
	// member lists format normally
	type L = { // prettier-ignore
		a: 1;
		b: 2;
	};
	interface A { // prettier-ignore
		a: 1;
		b: 2;
	}
</script>
