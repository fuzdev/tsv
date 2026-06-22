<script lang="ts">
	//
	// COMPARISONS (< is less-than operator)
	//

	// --- Member expressions ---
	const a1 = x.a < x.b;
	const a2 = x.a < 10;
	const a3 = 10 < x.a;
	const a4 = x.a.b < x.c.d;
	const a5 = x.a < y.b && x.c < y.d;

	// --- this member access ---
	const b1 = this.a < this.b;
	const b2 = this.a.b < this.c.d;

	// --- Private fields ---
	class C1 {
		#a = 0;
		#b = 10;
		fn() {
			return this.#a < this.#b;
		}
	}

	// --- Optional chaining ---
	const c1 = x?.a < 10;
	const c2 = x?.a?.b < x?.c?.d;

	// --- Non-null assertion ---
	const d1 = x!.a < 10;
	const d2 = x!.a!.b < x!.c!.d;

	// --- Array access (numeric index = comparison) ---
	const e1 = a[0] < a[1];
	const e2 = x.a[0] < x.b[1];

	// --- Array access (identifier index = comparison) ---
	const f1 = a[i] < 10;
	const f2 = a[i] < a[j];
	const f3 = x.a[i] < x.b[j];

	// --- After function call ---
	const g1 = fn() < 10;
	const g2 = x.fn() < 10;
	const g3 = fn().a < fn().b;

	// --- For loops ---
	for (let i = 0; i < a.length; i++) {}
	for (let i = 0; i < x.a.length; i++) {}

	// --- While loops ---
	while (i < a.length) {}
	while (x.a < x.b) {}

	// --- If statements ---
	if (x < y.a) {
	}
	if (x.a < y.b && a < b.c) {
	}

	// --- Ternary ---
	const h1 = x < y.a ? 1 : 2;
	const h2 = x.a < y.b ? x : y;

	// --- Return statements ---
	function fn1() {
		return x < a.length;
	}
	function fn2() {
		return x.a < x.b;
	}

	// --- Arrow functions ---
	const i1 = (x: number) => x < a.length;
	const i2 = (a: T, b: T) => a.x < b.x;
	const i3 = () => i < a.length;

	// --- Comparison as function argument ---
	fn(x.a < x.b);
	fn(x < y.a, z < w.b);
	a.filter((x) => x.a < 10);

	// --- Inside generic call (both generics AND comparison) ---
	fn<T>(a < b);
	x.fn<U>(x.a < y.b);

	// --- Comparison with comma (NOT type args) ---
	fn(a < x.a, b);
	fn(a < X.Y, b);
	[a < b, c < d];

	// --- After new expression ---
	const j1 = new A().a < 10;
	const j2 = new A().a < new B().b;

	// --- After type assertion ---
	const k1 = (x as T).a < 10;
	const k2 = (x as T).a < (y as U).b;

	// --- In object literal ---
	const l1 = { a: x < y };
	const l2 = { a: x.a < x.b, b: y < z };

	// --- In array literal ---
	const m1 = [a < b, c < d];
	const m2 = [x.a < x.b];

	// --- After await ---
	async function fn3() {
		const n1 = (await fn()).a < 10;
		const n2 = (await x.fn()).a < (await y.fn()).b;
	}

	// --- Prefix operators ---
	let o1 = 0;
	while (++o1 < a.length) {}

	// --- Static fields ---
	class C2 {
		static a = x < y;
		static b = x.a < 10;
	}

	// --- Arithmetic on right side ---
	const p1 = i < a.length - 1;
	const p2 = i < x.a / 2;
	const p3 = i < x.a * y.b;

	// --- Assignment context ---
	let q1: boolean;
	q1 = x < x.a;
	const q2 = (r = a < b.c);

	// --- Mixed comparison operators ---
	const r1 = a < b && c > d;
	const r2 = x <= y.a && x >= y.b;
	const r3 = x.a < x.b || x.c > x.d;

	//
	// GENERICS (< starts type arguments)
	//

	// --- Simple type args ---
	const s1 = fn<T>();
	const s2 = fn<T, U>();
	const s3 = fn<T, U, V>();

	// --- Generic with extends (in type declarations) ---
	function t1<T extends U>() {}
	function t2<T extends U, V extends W>() {}

	// --- Qualified type names ---
	fn<A.B>();
	fn<A.B, C.D>();
	new Map<A.B, C.D>();
	const u1: X<A.B> = x;

	// --- Nested generics ---
	const v1 = fn<T<U>>();
	const v2 = fn<Map<A, B>>();
	const v3 = new Map<A, Map<B, C>>();

	// --- Function type as type arg ---
	fn<(x: T) => U>();
	fn<() => void>();
	fn<(a: A, b: B) => C>();
	type Cb = X<(a: A) => B>;

	// --- Indexed access type ---
	type W1 = T[K];
	type W2 = T['a'];
	type W3 = X<T[K]>;

	// --- Type literal as type arg ---
	fn<{ a: T }>();
	fn<[T, U]>();
	fn<'a' | 'b'>();

	// --- Generic on method call ---
	const x1 = a.fn<T>();
	const x2 = a.b.fn<T, U>();
	const x3 = a.map<U>((x) => x);

	// --- Generic on new expression ---
	const y1 = new A<T>();
	const y2 = new A<T, U>();
	const y3 = new Map<A, B>();

	// --- Type alias with generics ---
	type Y1 = Array<T>;
	type Y2 = Map<A, B>;
	type Y3 = Promise<T>;

	// --- Generic in type position ---
	const z1: X<T> = x;
	const z2: Map<A, B> = x;
	function fn4<T>(x: X<T>): Y<T> {
		return x;
	}
</script>
