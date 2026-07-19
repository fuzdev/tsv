<script lang="ts">
	// comment between export and star
	export
	/* c1 */
	* from './a';

	// comment between export and type keyword
	export
	/* c2 */
	type * from './b';

	// comment between type keyword and star
	export type
	/* c3 */
	* from './c';

	// comment between star and from
	export type *
	/* c4 */
	from './d';

	// line comment between export and type keyword
	export // c5
		type * from './e';

	// line comment between export and star
	export // c6
		* from './f';

	// line comment between type keyword and star
	export type // c7
		* from './g';

	// line comment between star and from
	export * // c8
		from './h';
</script>
