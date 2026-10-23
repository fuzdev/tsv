<script lang="ts">
	// A line comment written before a module declaration's `;` trails after it — also when
	// the author left whitespace after the comment's text (the `;` on the next line)
	import a from './a' // c1	
	;
	import './d' // c2	
	;
	export { b } from './b' // c3	
	;
	export * from './c' // c4	
	;
	export { a } // c5	
	;
	export const e = 1 // c6	
	;
</script>
