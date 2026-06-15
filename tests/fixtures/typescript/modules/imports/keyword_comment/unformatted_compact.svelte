<script lang="ts">
	// block comment between import and source (bare import)
	import/* c */'a';

	// line comment between import and source (bare import)
	import// c
	'b';

	// block comment between from and source (with empty braces)
	import{}from/* c */'c';

	// line comment between from and source (with empty braces)
	import{}from// c
	'd';

	// empty braces with dangling comment (prettier relocates after from)
	import{/* c */}from'e';

	// type import with dangling comment (prettier relocates after from)
	import type{/* c */}from'f';

	// multiple block comments between from and source
	import{}from/* c1 *//* c2 */'g';

	// multiple line comments between from and source
	import{}from// c1
	// c2
	'h';

	// empty braces enclosing a comment with a `}` glyph (relocated after from)
	import{/* } */}from'j';

	// empty braces enclosing a comment with a `{` glyph (relocated after from)
	import{/* { */}from'k';

	// block comment between from and source (with specifiers)
	import{a}from/* c */'i';
</script>
