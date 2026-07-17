<script lang="ts">
	type Chain<T> =
		IsAny<T> extends true
			? never
			: [T] extends [boolean]
				? true
				: T extends Array<infer U>
					? Chain<U>
					: T extends Record<string, any>
						? { [K in keyof T]: Chain<T[K]> }[keyof T]
						: never;
</script>
