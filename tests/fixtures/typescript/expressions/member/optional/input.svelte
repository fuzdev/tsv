<script>
	// Basic optional chaining
	const a = obj?.prop;
	const b = arr?.[0];
	const c = fn?.();

	// Chained optional access
	const d = a?.b?.c;
	const e = obj?.prop?.nested?.deep;

	// Mixed chains (optional and regular)
	const f = a?.b.c;
	const g = a.b?.c;
	const h = a.b?.c.d;

	// Optional computed access
	const i = obj?.[key];
	const j = arr?.[index]?.value;
	const k = obj1?.[key1]?.[key2];

	// Optional method calls
	const l = obj?.method();
	const m = obj?.method?.();
	const n = obj1?.fn?.();

	// Chained method calls
	const o = obj?.first?.second();
	const p = obj1?.fn1()?.fn2?.();

	// Optional call with arguments
	const q = fn?.(1, 2, 3);
	const r = obj?.method?.(a, b, c);

	// Complex expressions
	const s = arr1?.[0]?.name;
	const t = obj1?.arr?.[0]?.value;
	const u = obj1?.obj2?.[key]?.trim?.();
</script>
