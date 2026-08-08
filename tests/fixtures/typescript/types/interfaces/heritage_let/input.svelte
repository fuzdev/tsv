<script lang="ts">
	// `let` is not a ReservedWord — the only bar on it as a name is a strict-mode
	// early error in BINDING positions, which a heritage element is not. So it
	// heads a heritage type reference like any other identifier
	interface A extends let {}
	class B implements let {}

	// It qualifies and takes type arguments like any other heritage name
	interface C extends let.Inner {}
	interface D extends let<string> {}
</script>
