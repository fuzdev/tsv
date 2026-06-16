<script lang="ts">
	// block comment between export and the named-specifier brace
	export /* c1 */ {a} from './a';

	// line comment between export and the named-specifier brace
	export // c2
	{b} from './b';
</script>
