<script lang="ts">
	// `yield` heads a heritage type reference: a heritage element is an
	// `IdentifierReference`, and outside a generator the only bar on `yield` there
	// is a strict-mode early error, which tsv defers
	interface A extends yield {}
	class B implements yield {}

	// It qualifies and takes type arguments like any other heritage name
	interface C extends yield.Inner {}
	interface D extends yield<string> {}
</script>
