<script lang="ts">
	// block comment between import and the named-specifier brace
	import /* c1 */ {a} from './a';

	// line comment between import and the named-specifier brace
	import // c2
		{b} from './b';
</script>
