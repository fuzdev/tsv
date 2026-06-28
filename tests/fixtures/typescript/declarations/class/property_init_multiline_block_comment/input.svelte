<script lang="ts">
	class C {
		// Multiline block comment before init: breaks `=`, value stays glued to `*/`
		foo =
			/*
			 * c
			 */ bar;
		// Single-line block comment: stays inline
		baz = /* c */ qux;
	}
</script>
