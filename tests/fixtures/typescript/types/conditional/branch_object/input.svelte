<script lang="ts">
	type Short<T> = T extends string ? { a: number } : never;
	type Obj<T> = T extends string
		? {
				aaaaaaaaaaaaaa: number;
				bbbbbbbbbbbbbb: string;
				cccccccccccccc: boolean;
				dddddddddddddd: symbol;
			}
		: never;
</script>
