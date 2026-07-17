<script lang="ts">
	type Short<T> = T extends string ? { a: T } : never;
	type Mapped<T> = T extends Record<PropertyKey, unknown> | unknown[]
		? {
				[K in keyof T]?: T[K] extends Record<PropertyKey, unknown> | unknown[]
					? Mapped<T[K]>
					: T[K];
			}
		: T | undefined;
</script>
