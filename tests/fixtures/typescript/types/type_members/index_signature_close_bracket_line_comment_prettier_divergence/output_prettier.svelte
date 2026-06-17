<script lang="ts">
	// Line comments in the key-type→`]` gap force the bracket to break and are
	// preserved before `]`: a same-line comment trails the type (`// b`), an
	// own-line comment stays on its own line (`// d`). Prettier keeps the bracket
	// inline and relocates an own-line comment to after `]` (`[key: string] // d`).
	interface A {
		[
			key: string // b
		] // d
		: number;
	}
</script>
