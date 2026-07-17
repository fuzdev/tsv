<script lang="ts">
	type Short<P> = Base & ({} extends P ? { a: number } : { b: string });
	type Cond<P> = Base &
		({} extends SomeLongerPropsTypeName
			? { aaaaaaaaaaaa: number; bbbbbbbbbbbb: string }
			: { cccccccccccc: symbol });
</script>
