<script lang="ts">
	// A method signature keyed `new` keeps its quotes: bare `new` before a parameter list or
	// type parameters starts a construct signature, a different member
	interface A {
		"new"(): void;
		"new"<T>(a: T): A;
		new (): A;
		new <T>(a: T): T;
		(): void;
		[k: string]: unknown;
	}
	type B = {
		"new"(a: number): B;
		"new"<T>(): T;
	};
	let c: { "new"(): void };
	function fn(a: { "new"(): void }) {}

	// An optional method and an accessor keyed `new` keep their quotes too
	interface D {
		"new"?(): void;
		"new"?<T>(): T;
	}
	type E = {
		get "new"(): number;
		set "new"(v: number);
	};

	// A property keyed `new`, other method keys, and a `new` method of a class or an object
	// literal unquote
	interface F {
		new: string;
		new2(): void;
		news(): void;
		constructor(): void;
		get(): void;
		readonly(): void;
	}
	type G = {
		readonly new: () => void;
		new?: new () => G;
	};
	declare class H {
		new(): void;
		static new(): void;
	}
	class N {
		new() {}
	}
	abstract class J {
		abstract new(): void;
	}
	const k = {
		new() {},
		get new2() {
			return 1;
		}
	};

	// Comments around a kept key
	interface M {
		/* c */ "new"(): void;
		"new" /* c */(): void;
		// c
		"new"<T>(): T; // c
	}

	// An escaped key stays as written
	interface L {
		'ne\u0077'(): void;
	}
</script>
