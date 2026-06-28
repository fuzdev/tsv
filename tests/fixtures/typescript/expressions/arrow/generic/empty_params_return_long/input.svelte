<script lang="ts">
	// Short - whole signature fits inline (contrast)
	const short = <T, U>(): { a: T; b: U } => null as any;

	// Boundary: signature fits inline at exactly 100 chars - stays on one line
	const fitA = <T, U>(): { a: Aaaaaaaa<T>; bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: U } => null as any;

	// Boundary: signature at 101 chars - body breaks after `=>`, type params stay inline
	const fitB = <T, U>(): { a: Aaaaaaaa<T>; bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: U } =>
		null as any;

	// Empty params + breaking object return: type params stay inline, object expands
	const objReturn = <T, U>(): {
		firstPropertyAaaaaaaa: Aaaaaaaaaaaa<T>;
		secondPropertyB: (v: U) => void;
	} => null as any;

	// Constrained single param + breaking object return: type param stays inline
	const constrained = <T extends BaseConstraintName>(): {
		firstPropertyAaaa: Aaaaaaaaaaaa<T>;
		secondProp: T;
	} => null as any;

	// Empty params + breaking union return: type params stay inline, union hangs after colon
	const unionReturn = <T, U>():
		AaaaaaaaaaaaaaaaaaaaaaaaaaName<T> | BbbbbbbbbbbbbbbbbbbbbbName<U> | Cccccccc<T> => null as any;
</script>
