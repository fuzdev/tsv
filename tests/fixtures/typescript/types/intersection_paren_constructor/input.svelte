<script lang="ts">
	type Short = (new () => Comp) & { a: number };
	type Ctor<Comp> = (new (
		options: ConstructorOptions<Comp extends Base<infer P> ? P : never>
	) => Comp) & { a: number };
</script>
