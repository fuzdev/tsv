<script lang="ts">
	// comment between export and star
	export * from /* c1 */ './a';

	// comment between export and type keyword
	export type * from /* c2 */ './b';

	// comment between type keyword and star
	export type * from /* c3 */ './c';

	// comment between star and from
	export type * from /* c4 */ './d';

	// line comment between export and type keyword
	export type * from // c5
	'./e';

	// line comment between export and star
	export * from // c6
	'./f';

	// line comment between type keyword and star
	export type * from // c7
	'./g';

	// line comment between star and from
	export * from // c8
	'./h';
</script>
