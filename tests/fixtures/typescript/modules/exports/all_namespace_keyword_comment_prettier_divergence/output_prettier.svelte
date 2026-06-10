<script lang="ts">
	// comment between star and as
	export * as /* c1 */ ns1 from './a';

	// comment between star and as (type re-export)
	export type * as /* c2 */ ns2 from './b';

	// line comment between star and as
	export * as // c3
	ns3 from './c';

	// comment between as and binding — dual-stable, both formatters keep
	export * as /* c4 */ ns4 from './d';

	// comment between binding and from — dual-stable, both formatters keep
	export * as ns5 /* c5 */ from './e';
</script>
