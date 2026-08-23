<script lang="ts">
	const a = async <T extends A>(x /* c1 */): T => x;
	const b = async <T extends A>(x = /* c2 */ 1): T => x;
	const c = async /* c3 */ <T extends A>(x): T => x;
	const d = async <T extends A>(x): /* c4 */ T => x;
</script>
