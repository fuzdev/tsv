<script lang="ts">
	const x = {
		// factory 2-call chain with long arg - breaks inside call, not at colon
		access: A.fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa').optional(),

		// factory 2-call chain with longer callee - same behavior
		longer: AAAAAA.describe('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa').optional(),

		// factory 2-call chain with short arg - fits on one line
		short1: B.fn('short').optional(),

		// short key - breaks inside call regardless of chain analysis
		prop: A.fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa').optional(),
	};
</script>
