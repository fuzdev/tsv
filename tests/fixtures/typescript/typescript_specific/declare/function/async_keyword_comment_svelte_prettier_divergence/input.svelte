<script lang="ts">
	declare /* c1 */ async function fn(): Promise<void>;

	declare async /* c2 */ function generic<T>(x: T): Promise<T>;

	declare async function* /* c3 */ both(): AsyncIterator<number>;
</script>
