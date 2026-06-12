<script lang="ts">
	// named specifiers, block comment before from
	export {a /* c1 */} from './a';

	// named specifiers, line comment before from
	export {
		b, // c2
	} from './b';

	// multiple named specifiers, block comment before from
	export {c, d /* c3 */} from './c';
</script>
