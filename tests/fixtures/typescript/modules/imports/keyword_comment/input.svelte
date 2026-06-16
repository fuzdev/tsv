<script lang="ts">
	// block comment between import and source (bare import)
	import /* c */ 'a';

	// block comment between from and source (with empty braces)
	import {} from /* c */ 'c';

	// empty braces with dangling comment (prettier relocates after from)
	import {} from /* c */ 'e';

	// type import with dangling comment (prettier relocates after from)
	import type {} from /* c */ 'f';

	// multiple block comments between from and source
	import {} from /* c1 */ /* c2 */ 'g';

	// empty braces enclosing a comment with a `}` glyph (relocated after from)
	import {} from /* } */ 'j';

	// empty braces enclosing a comment with a `{` glyph (relocated after from)
	import {} from /* { */ 'k';

	// block comment between from and source (with specifiers)
	import {a} from /* c */ 'i';
</script>
