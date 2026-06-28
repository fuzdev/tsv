<script lang="ts">
	enum E {
		// Multiline block comment before member init: value stays glued to `*/`
		A = /*
		 * c
		 */ 1,
		// Single-line block comment: stays inline
		B = /* c */ 2
	}
</script>
