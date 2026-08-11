<script lang="ts">
	declare async function fn(): Promise<void>;

	declare async function generic<T>(x: T): Promise<T>;

	declare async function* both(): AsyncIterator<number>;

	export declare async function exported(): Promise<void>;
</script>
