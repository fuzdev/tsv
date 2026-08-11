<script lang="ts">
	declare function* gen(): Iterator<number>;

	declare function* generic<T>(x: T): Iterator<T>;

	declare function* fn(x: string): Iterable<string>;
	declare function* fn(x: number): Iterable<number>;

	export declare function* exported(): Iterator<void>;
</script>
