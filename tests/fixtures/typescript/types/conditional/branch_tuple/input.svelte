<script lang="ts">
	type Short<T> = T extends string ? [a: number] : [b: string];
	type Tuple<Props> = {} extends Props
		? [
				component: Comp extends Base ? DerivedComponentType<Comp> : Comp,
				options: { a: OmitType; b: MapType }
			]
		: [component: Comp, options: { a: OmitType; b: MapType }];
</script>
