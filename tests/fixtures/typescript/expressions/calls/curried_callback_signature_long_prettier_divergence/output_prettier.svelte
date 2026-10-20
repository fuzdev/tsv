<script lang="ts" module>
	// A callback hugged to a curried callee prints its signature flat on the callee's line. At
	// 100 that head line holds at every position.
	export const aaaaaaaaaaaaaa = memoize(1000)((data: Data, id: string, skip: boolean): string[] => {
		return [id];
	});
	const aaaaaaaaaaaaaaaaaaaaa = memoize(1000)((data: Data, id: string, skip: boolean): string[] => {
		return [id];
	});
	let aaaaaaaaaaaaaaaaaaaaaaa = memoize(1000)((data: Data, id: string, skip: boolean): string[] => {
		return [id];
	});
	obj.aaaaaaaaaaaaaaaaaaaaaaa = memoize(1000)((data: Data, id: string, skip: boolean): string[] => {
		return [id];
	});
	module.exports = memoizeeeeeeeeeeeeee(1000)((data: Data, id: string, skip: boolean): string[] => {
		return [id];
	});
	export const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = memoize(1000)((path: string) => {
		return [path];
	});

	// At 101 every argument breaks out, the signature kept on one line, at every position: an
	// exported or plain `const`, a `let`, a member assignment, `module.exports =`, `export default`.
	export const aaaaaaaaaaaaaaa = memoize(1000)((
		data: Data,
		id: string,
		skip: boolean
	): string[] => {
		return [id];
	});

	const aaaaaaaaaaaaaaaaaaaaaa = memoize(1000)((
		data: Data,
		id: string,
		skip: boolean
	): string[] => {
		return [id];
	});

	let aaaaaaaaaaaaaaaaaaaaaaaa = memoize(1000)(
		(data: Data, id: string, skip: boolean): string[] => {
			return [id];
		}
	);

	obj.aaaaaaaaaaaaaaaaaaaaaaaa = memoize(1000)(
		(data: Data, id: string, skip: boolean): string[] => {
			return [id];
		}
	);

	module.exports = memoizeeeeeeeeeeeeeee(1000)((
		data: Data,
		id: string,
		skip: boolean
	): string[] => {
		return [id];
	});

	export default memoizeeeeeeeeeeeeeeeee(1000)((
		data: Data,
		id: string,
		skip: boolean
	): string[] => {
		return [id];
	});

	export const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = memoize(1000)((
		path: string
	) => {
		return [path];
	});

	// When the inner call takes more arguments than the outer one, the inner list breaks first;
	// a signature too long for its own line then breaks its parameters.
	const aaaaaaaa = memoize(
		1000,
		a
	)((
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: B,
		ccccccccccccccccccccccccc: string,
		d: boolean
	): string[] => {
		return [c];
	});
</script>
