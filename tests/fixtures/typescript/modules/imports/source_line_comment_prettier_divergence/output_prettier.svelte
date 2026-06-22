<script lang="ts">
	// bare import: line comment before source indents the source
	import // c
	'a';

	// bare import: line comment text contains brace-like glyphs
	import // {} from
	'./b';

	// empty braces: line comment between from and source
	import {} from // c
	'./c';

	// empty braces: multiple line comments between from and source
	import {} from // c1
	// c2
	'./d';

	// named specifiers: line comment between from and source
	import {
		a // c
	} from './e';

	// default binding: line comment between from and source
	import Foo from './f'; // c
</script>
