<script lang="ts">
	// A single parameter hugs (stays flat) while a breaking object return type
	// expands, matching class methods and prettier. The trailing method keeps the
	// object literal multiline, so each signature sits at its own indent.
	const o = {
		// 100 chars - signature fits, stays inline
		m1(pAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: string): { a: () => number; b: (v: number) => void } {},
		// 101 chars - single param hugs, object return type expands
		m2(pAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA: string): {
			a: () => number;
			b: (v: number) => void;
		} {},
		k() {}
	};

	// Function expressions - same behavior via the shared signature path.

	// 100 chars - signature fits, stays inline
	const f1 = function (pAAAAAAAAAAAAAAAAAA: string): { a: () => number; b: (v: number) => void } {};
	// 101 chars - single param hugs, object return type expands
	const f2 = function (pAAAAAAAAAAAAAAAAAAA: string): {
		a: () => number;
		b: (v: number) => void;
	} {};
</script>
