<script lang="ts">
	// own-line comment before a declare-function `;` moves to its own line after `;`
	declare function f(): void
	/* c1 */
	;

	// same-line block stays before the `;`
	declare function g(): void /* c2 */;
</script>
